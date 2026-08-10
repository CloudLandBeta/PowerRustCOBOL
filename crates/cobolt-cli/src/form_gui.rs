// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `rcrun run-form` — standalone GUI form runner.
//!
//! Runs a form program (a `.cfrm` layout + its generated `.cbl`) in **its own
//! process and event loop**. The IDE spawns this for Run Form, so the IDE
//! stays idle while the form does the work — and what you test is exactly
//! what `rcrun build` ships, because since spec 042 the window is the SAME
//! shared form host (`cobolt-form-host`) a compiled application runs. This
//! file is only the run-form glue: argument parsing, parse/check diagnostics,
//! the debug-wired interpreter thread, and on-disk theme-pack discovery — the
//! per-host seam (042 R30).
//!
//! ```text
//! rcrun run-form <form.cfrm> <program.cbl>
//! ```
//!
//! DISPLAY output goes to stdout (the IDE pipes it into its Output pane);
//! parse/semantic/runtime errors go to stderr and yield a non-zero exit code.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, Mutex};

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::parse;
use cobolt_runtime::{FormEvent, Interpreter, StateUpdate};
use cobolt_semantic::analyze;

// ── Shared host pieces (spec 042) ─────────────────────────────────────────────
// Control state, seeding, diagnostics and the host itself all live in
// `cobolt-form-host`; every host consumes the same code.
use cobolt_form_host::diagnostics::{env_flag, write_diagnostics_dump};
use cobolt_form_host::flatten_controls;
use cobolt_form_host::seeding::{build_object_seed, resolve_api_keys};
use cobolt_form_host::state::{state_entry_mut, CtrlState};

/// 038 — resolve the `--fx-entrance/--fx-exit/--fx-restore` args. `killed`
/// (the `PRC_NO_WINDOW_FX=1` kill-switch) zeroes everything regardless of
/// what the args say, so any caller can force instant windows.
fn parse_fx_args(
    args: &[String],
    killed: bool,
) -> (
    cobolt_forms::window_fx::FxSpec,
    cobolt_forms::window_fx::FxSpec,
    bool,
) {
    if killed {
        return (
            cobolt_forms::window_fx::FxSpec::default(),
            cobolt_forms::window_fx::FxSpec::default(),
            false,
        );
    }
    let fx_arg = |name: &str| -> cobolt_forms::window_fx::FxSpec {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .map(|v| cobolt_forms::window_fx::FxSpec::parse(v))
            .unwrap_or_default()
    };
    (
        fx_arg("--fx-entrance"),
        fx_arg("--fx-exit"),
        args.iter().any(|a| a == "--fx-restore"),
    )
}

pub fn cmd_run_form(args: &[String]) {
    let (cfrm_path, cbl_path) = match (args.first(), args.get(1)) {
        (Some(a), Some(b)) => (PathBuf::from(a), PathBuf::from(b)),
        _ => {
            eprintln!(
                "usage: rcrun run-form <form.cfrm> <program.cbl> [--theme-default <id>] [--icon <image>] [--debug]"
            );
            process::exit(2);
        }
    };
    // Project-level default theme id, forwarded by the IDE (per-form overrides
    // in the .cfrm still win — same resolution the designer canvas uses).
    let theme_default: Option<String> = args
        .iter()
        .position(|a| a == "--theme-default")
        .and_then(|i| args.get(i + 1))
        .cloned();
    let icon_path: Option<PathBuf> = args
        .iter()
        .position(|a| a == "--icon")
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from);
    // Debug mode: the IDE (or any future remote host — Android/iOS) controls
    // the interpreter over stdin/stdout. Commands arrive as `@DBG <json>` lines
    // on stdin; DebugEvents leave as `@DBG <json>` lines on stdout. Plain
    // stdout lines remain DISPLAY output. The program starts paused at line 1.
    let debug_mode = args.iter().any(|a| a == "--debug");
    // When ANY diagnostic is enabled, the IDE forwards `--diagnostics-dump
    // <project>`; write the detailed per-control dump once, at launch.
    let diagnostics_dump_project: Option<String> = args
        .iter()
        .position(|a| a == "--diagnostics-dump")
        .and_then(|i| args.get(i + 1))
        .cloned();
    // 038 — window effects, resolved by the IDE (project × form opt-out ×
    // kill-switch). `PRC_NO_WINDOW_FX` overrides even explicit args so any
    // caller (automation, CI) can force instant windows — read with the one
    // shared truthiness rule (spec 042 R28).
    let fx_killed = env_flag("PRC_NO_WINDOW_FX");
    let (fx_entrance, fx_exit, fx_restore) = parse_fx_args(args, fx_killed);

    // ── Load the form layout ──────────────────────────────────────────────────
    let form = match cobolt_forms::load_form(&cfrm_path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("run-form: cannot load {}: {e}", cfrm_path.display());
            process::exit(1);
        }
    };

    if let Some(project) = diagnostics_dump_project.as_deref() {
        write_diagnostics_dump(project, &form);
    }

    // ── Parse + analyse the COBOL program ─────────────────────────────────────
    let source = match std::fs::read_to_string(&cbl_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("run-form: cannot read {}: {e}", cbl_path.display());
            process::exit(1);
        }
    };
    let fmt = SourceFormat::detect(&source);
    let tokens = tokenize(&source, fmt);
    let parse_result = parse(tokens);
    let mut hard_errors = false;
    for d in &parse_result.diagnostics {
        let sev = match d.severity {
            cobolt_parser::Severity::Error => {
                hard_errors = true;
                "error"
            }
            cobolt_parser::Severity::Warning => "warning",
        };
        eprintln!(
            "{}:{}:{}: {sev}: {}",
            cbl_path.display(),
            d.span.line,
            d.span.col,
            d.message
        );
    }
    let program = match parse_result.program {
        Some(p) if !hard_errors => p,
        _ => {
            eprintln!("run-form: aborting — parse errors found.");
            process::exit(1);
        }
    };
    let sem = analyze(&program);
    for d in &sem.diagnostics {
        use cobolt_semantic::Severity;
        let sev = match d.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Info => "info",
        };
        eprintln!(
            "{}:{}:{}: {sev}: {}",
            cbl_path.display(),
            d.span.line,
            d.span.col,
            d.message
        );
    }
    if !sem.is_ok() {
        eprintln!("run-form: aborting — semantic errors found.");
        process::exit(1);
    }

    // ── Flatten controls + initial state + object-registry seed ──────────────
    let mut flat: Vec<cobolt_forms::Control> = Vec::new();
    flatten_controls(&form.controls, &mut flat);
    flat.sort_by_key(|c| c.z_order);

    let mut state: HashMap<String, CtrlState> = HashMap::new();
    for c in &flat {
        state.insert(c.id.clone(), CtrlState::from_control(c));
    }

    // Seed the interpreter's visual-object registry with every control's
    // designed properties, so property references and method getters return
    // the configured values before any setter runs — via the shared builder
    // (spec 042 R20), so run-form and compiled applications seed identically.
    let (maps_api_key, search_api_key) = resolve_api_keys();
    let seed = build_object_seed(
        &form,
        &flat,
        maps_api_key.as_deref(),
        search_api_key.as_deref(),
    );

    // ── Interpreter thread ────────────────────────────────────────────────────
    let (ev_tx, ev_rx) = mpsc::channel::<FormEvent>();
    let (input_tx, input_rx) = mpsc::channel::<StateUpdate>();
    let (state_tx, state_rx) = mpsc::channel::<StateUpdate>();
    let (display_tx, display_rx) = mpsc::channel::<String>();

    let pending = Arc::new(AtomicUsize::new(0));
    let finished = Arc::new(AtomicBool::new(false));
    let error_slot: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));

    // ── Remote debug wiring (`--debug`) ──────────────────────────────────────
    // stdin `@DBG <json RemoteDebugCmd>` lines → interpreter debug channels;
    // interpreter DebugEvents → stdout `@DBG <json DebugEvent>` lines. The
    // same line protocol can later ride adb/ssh for Android/iOS debuggees.
    let debug_wiring = if debug_mode {
        use cobolt_runtime::{new_breakpoints, DebugEvent, RemoteDebugCmd};

        let (dbg_cmd_tx, dbg_cmd_rx) = mpsc::channel::<cobolt_runtime::DebugCmd>();
        let (dbg_ev_tx, dbg_ev_rx) = mpsc::channel::<DebugEvent>();
        let breakpoints = new_breakpoints();

        // stdin reader: parse and dispatch remote debug commands.
        {
            let bps = Arc::clone(&breakpoints);
            std::thread::spawn(move || {
                use std::io::BufRead;
                let stdin = std::io::stdin();
                for line in stdin.lock().lines().map_while(Result::ok) {
                    let Some(json) = line.strip_prefix("@DBG ") else {
                        continue;
                    };
                    match serde_json::from_str::<RemoteDebugCmd>(json) {
                        Ok(RemoteDebugCmd::Cmd(c)) => {
                            if dbg_cmd_tx.send(c).is_err() {
                                break;
                            }
                        }
                        Ok(RemoteDebugCmd::SetBreakpoints(lines)) => {
                            if let Ok(mut guard) = bps.lock() {
                                *guard = lines.into_iter().collect();
                            }
                        }
                        // TODO(debug-user-scope): wire the interpreter's user-only
                        // stepping once `Interpreter::set_debug_user_scope` lands
                        // (parked "hide generated code" feature). Accepted here so
                        // the child never rejects the command; currently a no-op.
                        Ok(RemoteDebugCmd::SetUserScope { .. }) => {}
                        Err(e) => eprintln!("run-form: bad @DBG command: {e}"),
                    }
                }
            });
        }

        // event pump: interpreter → stdout (whole lines; println! locks stdout,
        // so interleaving with DISPLAY output stays line-atomic).
        std::thread::spawn(move || {
            use std::io::Write;
            for ev in dbg_ev_rx.iter() {
                match serde_json::to_string(&ev) {
                    Ok(json) => {
                        println!("@DBG {json}");
                        let _ = std::io::stdout().flush();
                    }
                    Err(e) => eprintln!("run-form: cannot serialize DebugEvent: {e}"),
                }
            }
        });

        Some((dbg_cmd_rx, dbg_ev_tx, breakpoints))
    } else {
        None
    };

    // ── 037 window supervisor (single-window host) ────────────────────────────
    // The interpreter's OpenForm*/handle/window methods talk to a real
    // FormSupervisor owned by the GUI loop. FormState close vetoes, window
    // commands and onCloseRejected/onFullScreenChanged all work here; the
    // SpawnWindow execution (real child viewports) lands with the T1
    // multi-viewport spike's findings — until then a child spawn is released
    // immediately (logged), so callers never deadlock.
    let (form_req_tx, form_req_rx) =
        mpsc::channel::<cobolt_runtime::form_host::FormRequest>();
    let (closed_tx, closed_rx) = mpsc::channel::<String>();
    let form_object = form.name.trim().to_ascii_uppercase();

    {
        let finished = Arc::clone(&finished);
        let error_slot = Arc::clone(&error_slot);
        let pending = Arc::clone(&pending);
        let form_req_tx = form_req_tx.clone();
        let form_object = form_object.clone();
        std::thread::spawn(move || {
            let mut interp = Interpreter::new_with_channels(program, ev_rx, state_tx, display_tx);
            interp.set_input_channel(input_rx);
            interp.set_event_counter(pending);
            interp.set_form_host(
                form_req_tx,
                cobolt_runtime::form_host::ROOT_HANDLE,
                &form_object,
                closed_rx,
            );
            interp.seed_objects(seed);
            if let Some((cmd_rx, ev_tx, bps)) = debug_wiring {
                interp.attach_debug_channels(cmd_rx, ev_tx, bps);
            }
            match interp.run() {
                Ok(()) => {}
                Err(e) if e.is_exit_signal() => {}
                Err(e) => {
                    if let Ok(mut slot) = error_slot.lock() {
                        *slot = Some(e.to_string());
                    }
                }
            }
            finished.store(true, Ordering::Relaxed);
        });
    }

    // ── Theme + glass parity with the designer canvas (spec 017/007) ──────────
    // Resolve the form's theme (per-form override ?? project default ?? Liquid
    // Glass) against packs discovered next to the executable — the same lookup
    // the IDE performs — so the standalone window paints identically. The pack
    // SOURCE (disk here, embedded art in a compiled application) is the
    // per-host part; the resolution rule is not.
    let theme_id =
        cobolt_forms::theme::resolve_theme_id(form.theme.as_deref(), theme_default.as_deref());
    // A procedural theme has no pack on disk — do not go looking for one.
    let theme_pack: Option<Arc<cobolt_forms::theme_pack::ThemePack>> =
        if cobolt_forms::theme::ThemeCatalog::procedural_ids().contains(&theme_id.as_str()) {
            None
        } else {
            let themes_dir = std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|d| d.join("assets/themes")))
                .filter(|d| d.is_dir())
                .unwrap_or_else(|| PathBuf::from("assets/themes"));
            cobolt_forms::theme_pack::discover_packs(&themes_dir)
                .into_iter()
                .find(|p| p.id == theme_id)
                .map(Arc::new)
        };
    let surface_style = cobolt_forms::paint::SurfaceStyle::from_theme_id(&theme_id);

    // ── GUI event loop (own process — the IDE stays idle) ─────────────────────
    // The window itself is the SHARED form host (spec 042 R1): everything from
    // here — viewport assembly, 038 effects, 037 lifecycle, state routing —
    // is the same code a compiled application runs. This glue's own pieces
    // ended above: args, parse/check, the debug-wired interpreter thread and
    // disk theme discovery (the R30 seam).
    //
    // 049 R2/R3 — a form carrying a SideMenu control starts in SHELL mode
    // (MenuPane + breadcrumb + ContentPane, one window); any other form —
    // including one with a MenuBar — keeps the classic one-window mode
    // exactly as before.
    let shell_mode = form.has_side_menu();
    let root_menu = if shell_mode {
        form.side_menu_control_id().and_then(|ctrl_id| {
            let dir = cfrm_path.parent()?;
            let yaml = cobolt_forms::menu::menu_yaml_path(dir, &ctrl_id);
            let def = cobolt_forms::menu::load_menu(&yaml).ok()?;
            Some((ctrl_id, def))
        })
    } else {
        None
    };
    let config = cobolt_form_host::FormHostConfig {
        form,
        flat,
        state,
        ev_tx,
        input_tx,
        state_rx,
        display_rx,
        pending,
        finished: Arc::clone(&finished),
        form_req_rx,
        closed_tx,
        fx_entrance,
        fx_exit,
        fx_restore,
        theme_pack,
        surface_style,
        icon_path,
        // R17 — a blank designed title stays blank under run-form, exactly as
        // it always has (the branded fallback belongs to built applications).
        title_fallback: String::new(),
        // run-form is the classic one-window mode (049 R3); run_shell forces
        // Pane itself.
        surface: cobolt_form_host::Surface::Window,
        hooks: Box::new(cobolt_form_host::NoHooks),
    };
    if shell_mode {
        cobolt_form_host::shell::run_shell(config, root_menu);
    } else {
        cobolt_form_host::run(config);
    }

    // Surface a runtime error (if any) after the window closes.
    let runtime_error = error_slot.lock().ok().and_then(|mut s| s.take());
    if let Some(e) = runtime_error {
        eprintln!("run-form: runtime error: {e}");
        process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cobolt_forms::{Control, ControlType};

    /// 038 T6 — the `--fx-*` args round-trip through the parser, and the
    /// kill-switch zeroes them regardless of what the args say.
    #[test]
    fn fx_args_parse_and_kill_switch() {
        use cobolt_forms::window_fx::{Easing, WindowEffect};
        let args: Vec<String> = [
            "form.cfrm",
            "gen.cbl",
            "--fx-entrance",
            "matrix-rain:2000:ease-out",
            "--fx-exit",
            "fade:400:ease-in",
            "--fx-restore",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();

        let (ent, exit, restore) = parse_fx_args(&args, false);
        assert_eq!(ent.effect, WindowEffect::MatrixRain);
        assert_eq!(ent.duration_ms, 2000);
        assert_eq!(ent.easing, Easing::EaseOut);
        assert_eq!(exit.effect, WindowEffect::Fade);
        assert_eq!(exit.easing, Easing::EaseIn);
        assert!(restore);
        println!(
            "fx args: entrance={} exit={} restore={restore}",
            ent.format(),
            exit.format()
        );

        // Kill-switch: everything zeroed, args ignored.
        let (kent, kexit, krestore) = parse_fx_args(&args, true);
        assert!(!kent.is_active() && !kexit.is_active() && !krestore);
        // No args at all ⇒ inactive defaults.
        let (nent, nexit, nrestore) = parse_fx_args(&["a".to_string()], false);
        assert!(!nent.is_active() && !nexit.is_active() && !nrestore);
        println!("fx args killed/absent ⇒ inactive");
    }

    /// A repeating-group card member the interpreter writes to before the id
    /// exists in `state` must stay VISIBLE. With a derived `Default` the entry
    /// was born `visible: false`, so the renderer skipped exactly the controls a
    /// data binding had just populated — databound cards blank in the run form
    /// while the DataGrid (pre-seeded key) painted fine.
    #[test]
    fn new_instance_entry_is_visible_and_keeps_designed_props() {
        let mut label = Control::new("Label-1", ControlType::Label, 0, 0);
        label.set_prop("Caption", cobolt_forms::PropValue::from("designed"));
        label.parent = Some("GroupBox-2".into());
        let controls = vec![label];

        let mut state: HashMap<String, CtrlState> = HashMap::new();
        let key = cobolt_forms::render::member_instance_id("GroupBox-2", "Label-1", 1);
        state_entry_mut(&mut state, &controls, &key).set("Caption", "Leonardo DiCaprio".into());

        let entry = &state[&key];
        assert!(entry.visible, "card-instance state entry must start visible");
        assert!(entry.enabled, "card-instance state entry must start enabled");
        assert_eq!(entry.props.get("Caption").map(String::as_str), Some("Leonardo DiCaprio"));
    }

    /// An id with no matching designed control (an unknown write) still lands in
    /// a visible entry rather than a silently hidden one.
    #[test]
    fn unknown_id_entry_defaults_to_visible() {
        let mut state: HashMap<String, CtrlState> = HashMap::new();
        state_entry_mut(&mut state, &[], "Nope-1").set("Caption", "x".into());
        assert!(state["Nope-1"].visible);
        assert!(state["Nope-1"].enabled);
    }

    /// A member designed hidden stays hidden when its instance entry is created.
    #[test]
    fn instance_entry_inherits_designed_visibility() {
        let mut label = Control::new("Label-9", ControlType::Label, 0, 0);
        label.visible = false;
        let controls = vec![label];
        let mut state: HashMap<String, CtrlState> = HashMap::new();
        let key = cobolt_forms::render::member_instance_id("GroupBox-2", "Label-9", 3);
        state_entry_mut(&mut state, &controls, &key).set("Caption", "v".into());
        assert!(!state[&key].visible);
    }
}
