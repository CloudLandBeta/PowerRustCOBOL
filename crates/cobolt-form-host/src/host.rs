// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The form host itself (spec 042 R1): the `eframe::App` that paints a
//! designed form, plays the spec-038 window effects, runs the spec-037
//! lifecycle, routes state and events, and paces frames. Moved verbatim from
//! `rcrun run-form` (`cobolt-cli/src/form_gui.rs`), which was the
//! behaviourally complete host; both live surfaces are thin glue over this.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{mpsc, Arc};

use cobolt_runtime::{FormEvent, StateUpdate};

use crate::state::{state_entry_mut, CtrlState, LiveState};

/// How long an effect actually plays. Every effect but MatrixRain takes the
/// duration it was configured with; MatrixRain's falling lines are scheduled
/// in real milliseconds, one per 25–50 ms beat, so its configured value is a
/// FLOOR — the effect runs as long as its own schedule needs (operator,
/// 2026-07-31: more lines at a wider beat, "mesmo que ultrapassasse o tempo").
pub fn fx_duration_ms(spec: &cobolt_forms::window_fx::FxSpec, width: f32) -> u32 {
    if spec.effect == cobolt_forms::window_fx::WindowEffect::MatrixRain {
        cobolt_forms::window_fx::matrix_effective_duration_ms(width, spec.duration_ms)
    } else {
        spec.duration_ms
    }
}

/// The window icon: the given path when it decodes, else the embedded
/// PowerRustCOBOL icon — every host window carries an icon.
pub fn load_host_icon(path: Option<&Path>) -> Option<egui::IconData> {
    path.and_then(|path| std::fs::read(path).ok())
        .and_then(|bytes| decode_icon(&bytes))
        .or_else(|| {
            decode_icon(include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../assets/images/powerrustcobol-icon.png"
            )))
        })
}

fn decode_icon(bytes: &[u8]) -> Option<egui::IconData> {
    let img = image::load_from_memory(bytes)
        .ok()?
        .resize_exact(256, 256, image::imageops::FilterType::Lanczos3)
        .into_rgba8();
    let (w, h) = img.dimensions();
    Some(egui::IconData {
        rgba: img.into_raw(),
        width: w,
        height: h,
    })
}

/// What a COBOL animation verb asked for. The interpreter turns `PLAY ANIMATION`,
/// `STOP-ANIMATION` and `PAUSE` into writes of these pseudo-properties on the
/// control object, which reach the GUI as ordinary state updates.
#[derive(Clone, Copy, PartialEq, Debug)]
enum AnimCommand {
    Play,
    Stop,
    Pause,
}

/// Map a state-update property name to its animation verb, if it is one.
fn anim_command(prop: &str) -> Option<AnimCommand> {
    match prop.trim() {
        p if p.eq_ignore_ascii_case("_PlayAnimation") => Some(AnimCommand::Play),
        p if p.eq_ignore_ascii_case("_StopAnimation") => Some(AnimCommand::Stop),
        p if p.eq_ignore_ascii_case("_PauseAnimation") => Some(AnimCommand::Pause),
        _ => None,
    }
}

/// The per-host seam (spec 042 R30) — the ONLY extension point. Everything a
/// host cannot express through [`FormHostConfig`] data goes through here, and
/// the list is deliberately short:
///
/// - the **compiled application** replays `EXEC RUST` block windows
///   (`cobolt_windows::show_all`) in [`HostHooks::per_frame`];
/// - `rcrun run-form` needs no hook at all ([`NoHooks`]).
pub trait HostHooks {
    /// Called once per frame, after the theme/glass state is installed and
    /// before anything else runs. Default: nothing.
    fn per_frame(&mut self, _ctx: &egui::Context) {}
}

/// The empty seam — a host with no per-host behaviour.
pub struct NoHooks;
impl HostHooks for NoHooks {}

/// Everything a glue layer supplies to run a form window. The glue owns the
/// interpreter thread (that is where the intentional per-host differences
/// live — the debugger channel in run-form, compiled-block registration in a
/// built application); the host owns the window.
/// 049 R18/R42 — where a host's form lives. `Window` is the historical mode:
/// the form owns an OS window, entrance/exit effects play, viewport commands
/// apply. `Pane` embeds the form in the application shell's ContentPane: the
/// SHELL owns the only window, so window-only behaviour is neutralised — no
/// effects (R18), no viewport commands, and nothing the form does can move or
/// resize its host (R42).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Surface {
    #[default]
    Window,
    Pane,
}

pub struct FormHostConfig {
    /// The designed form — window properties, backdrop, fx opt-out and the
    /// controls' designed tree all come from here.
    pub form: cobolt_forms::Form,
    /// 049 — own OS window, or embedded in the shell's ContentPane.
    pub surface: Surface,
    /// Flattened, z-sorted controls (see [`crate::flatten_controls`]).
    pub flat: Vec<cobolt_forms::Control>,
    /// Initial control state, seeded from the designed controls.
    pub state: HashMap<String, CtrlState>,
    /// UI → interpreter events.
    pub ev_tx: mpsc::Sender<FormEvent>,
    /// UI → interpreter live property values (slider drags, text edits, …).
    pub input_tx: mpsc::Sender<StateUpdate>,
    /// Interpreter → UI property updates.
    pub state_rx: mpsc::Receiver<StateUpdate>,
    /// Interpreter → UI DISPLAY lines.
    pub display_rx: mpsc::Receiver<String>,
    /// Events queued for the interpreter — lets the render Timer arm coalesce
    /// ticks against a backlog and drives the fast-repaint branch.
    pub pending: Arc<AtomicUsize>,
    /// Set by the interpreter thread when the program ends (STOP RUN or error).
    pub finished: Arc<AtomicBool>,
    /// Requests from the interpreter thread (OpenForm*, handle methods, …).
    pub form_req_rx: mpsc::Receiver<cobolt_runtime::form_host::FormRequest>,
    /// Broadcasts closed handles back to the interpreter (037 R24 NULLing).
    pub closed_tx: mpsc::Sender<String>,
    /// 051 — the sender side of `form_req_rx`, cloned into every spawned
    /// child interpreter so children can open forms and drive handles too.
    pub form_req_tx: mpsc::Sender<cobolt_runtime::form_host::FormRequest>,
    /// 051 R6 — resolves a form id to its design + program. The compiled
    /// binary reads its embedded tables; `rcrun`/the IDE read the project on
    /// disk. `None` = a single-form host: any open request fails visibly
    /// (R15) instead of silently dropping.
    pub form_source: Option<FormSource>,
    /// 051 — per-form theme resolution for spawned children (asset pack +
    /// procedural look). `None` ⇒ children paint procedural Liquid Glass.
    pub child_theme: Option<ChildThemeSource>,
    /// 051 — per-interpreter setup for spawned children (the compiled
    /// application registers its EXEC RUST blocks here; `rcrun` needs none).
    pub child_interpreter_setup:
        Option<std::sync::Arc<dyn Fn(&mut cobolt_runtime::interpreter::Interpreter) + Send + Sync>>,
    /// The indexed-file engine the application creates new files with — the
    /// one the glue gave its ROOT interpreter (`rcrun run-form
    /// --indexed-engine`, the project setting a compiled binary carries).
    /// Every child interpreter this host spawns gets the same engine, so a
    /// file created from a child form is the same format as one created from
    /// the main form. An existing file still opens with the engine that wrote
    /// it, whatever this says.
    pub indexed_engine: cobolt_runtime::indexed::IndexedEngine,
    /// 051 Q1 (operator ruling) — the ONE process-wide EXEC RUST object
    /// bridge, cloned into every child interpreter. `None` ⇒ children keep
    /// private bridges (single-form runs are unaffected either way).
    pub shared_rust_bridge:
        Option<std::sync::Arc<std::sync::Mutex<cobolt_runtime::rust_bridge::RustBridge>>>,
    /// 038 — entrance/exit effects, already resolved by the glue
    /// (project settings × the form's `WindowEffects` opt-out × the
    /// `PRC_NO_WINDOW_FX` kill switch).
    pub fx_entrance: cobolt_forms::window_fx::FxSpec,
    pub fx_exit: cobolt_forms::window_fx::FxSpec,
    /// Replay the entrance when the window is restored after minimize (R9).
    pub fx_restore: bool,
    /// Resolved asset-pack theme (None = built-in Liquid Glass). The SOURCE is
    /// per-host (disk discovery vs embedded art) — the resolution rule is not.
    pub theme_pack: Option<Arc<cobolt_forms::theme_pack::ThemePack>>,
    /// The procedural look the controls are painted in (spec 047). Resolved
    /// from the same theme id as `theme_pack`; `LiquidGlass` is the historical
    /// default, so a glue that does not set it renders exactly as before.
    pub surface_theme: std::sync::Arc<dyn cobolt_forms::surface_theme::SurfaceTheme>,
    /// Project icon path, if the glue has one (`--icon` / bundled asset).
    pub icon_path: Option<PathBuf>,
    /// Window title when the designed `form.title` is blank (spec 042 R17):
    /// run-form passes an empty string (a blank title stays blank, as ever);
    /// a compiled application passes `"{AppName} v{Version}"`.
    pub title_fallback: String,
    /// The per-host seam (R30).
    pub hooks: Box<dyn HostHooks>,
}

/// 051 R6 — how a host turns a form id into something it can run: the form's
/// design and its program. Per glue: the compiled binary looks up its
/// embedded `FORMS`/`PROGRAMS` tables; `rcrun` and the IDE read the project
/// from disk (regenerated `.cbl` beside the `.cfrm`).
pub type FormSource = Box<
    dyn Fn(&str) -> Result<(cobolt_forms::Form, cobolt_ast::program::Program), String> + Send,
>;

/// 051 — a spawned child form's theme, resolved by the glue that knows where
/// theme art lives (embedded vs `assets/themes/` on disk).
pub type ChildThemeSource = Box<
    dyn Fn(
            &cobolt_forms::Form,
        ) -> (
            Option<Arc<cobolt_forms::theme_pack::ThemePack>>,
            std::sync::Arc<dyn cobolt_forms::surface_theme::SurfaceTheme>,
        ) + Send,
>;

/// 051 — the closed-handle broadcast, for real. `HostAction::NotifyClosed`
/// was documented as "broadcast to every interpreter", but the transport was
/// one `mpsc` pair — strictly single-consumer. With one interpreter per
/// hosted form, every interpreter registers its own sender here and each
/// close reaches all of them, so every `windowHandler` NULLs (037 R24)
/// whichever form is holding it.
pub(crate) struct ClosedFanout(Vec<mpsc::Sender<String>>);

impl ClosedFanout {
    pub(crate) fn new(root: mpsc::Sender<String>) -> Self {
        Self(vec![root])
    }

    /// Register one more interpreter's receiver end.
    pub(crate) fn register(&mut self, tx: mpsc::Sender<String>) {
        self.0.push(tx);
    }

    /// Deliver `handle` to every registered interpreter. A dead receiver
    /// (its interpreter already ended) is simply skipped — closing is
    /// exactly when receivers die, so send errors here are ordinary.
    pub(crate) fn send(&self, handle: &str) {
        for tx in &self.0 {
            let _ = tx.send(handle.to_owned());
        }
    }
}

/// The window title rule (spec 042 R17): the DESIGNED title wins; the
/// fallback (the glue's choice — blank under run-form, branded in a compiled
/// application) shows only when the design left the title blank.
pub(crate) fn window_title(designed: &str, fallback: String) -> String {
    if designed.trim().is_empty() {
        fallback
    } else {
        designed.to_owned()
    }
}

/// 038 — what the entrance effect means for the window surface: a window that
/// plays an entrance drops its chrome for the duration (nothing sits still
/// while the effect animates), and effects that only move, scale or fade the
/// form's own face additionally get a SEE-THROUGH window so the form plays
/// loose on the desktop; the mask effects and MatrixRain paint over the whole
/// window by design and keep an opaque one. Returns
/// `(hide_chrome, transparent)`.
pub(crate) fn fx_window_flags(entrance: &cobolt_forms::window_fx::FxSpec) -> (bool, bool) {
    (
        entrance.is_active(),
        entrance.is_active() && entrance.effect.plays_over_desktop(),
    )
}

/// Build the window and run the form host to completion. Returns when the
/// window closes; the glue then reports interpreter errors its own way.
/// 051 R19/R28 — the painter the modal-block overlay goes through: `ui`'s
/// layer and clip rect, but a FRESH painter from the context, so it carries
/// none of the opacity an ancestor's `Ui::disable` multiplied in (egui 0.36:
/// `disabled_alpha`, 0.5, inherited by every child `Ui`). The overlay is the
/// deliberate "this form is waiting" signal and must paint at its designed
/// strength whether or not the shell disabled the whole face above it.
/// The scroll bars of a form's surface: floating — drawn over the form, taking
/// no room from its layout, so no existing form moves — but with the handle
/// VISIBLE whenever the form is larger than the surface. egui's floating
/// style hides it until the pointer finds the edge, so a form wider than a
/// shell's ContentPane showed no sign it could scroll sideways, and without a
/// trackpad could not be (operator, 2026-09-29). The bars live inside the
/// pane's own scroll area, so the SideMenu beside it never moves.
pub(crate) fn form_scroll_style() -> egui::style::ScrollStyle {
    let mut style = egui::style::ScrollStyle::floating();
    // Visible at rest, and thick enough to find with the mouse: egui's
    // floating bar is 2 px thin until hovered.
    style.dormant_handle_opacity = 0.6;
    style.floating_width = 5.0;
    style
}

pub(crate) fn overlay_painter(ui: &egui::Ui) -> egui::Painter {
    ui.ctx()
        .layer_painter(ui.layer_id())
        .with_clip_rect(ui.clip_rect())
}

/// The one grey layer a blocked form wears, in the strength its design chose
/// — the SAME fill wherever the blocked face is painted (the form's own
/// pane, and the shell's rail and breadcrumb around it), so `Greyed` never
/// reads as `SemiTransparent` on one part of the window and not another.
pub(crate) fn modal_overlay_fill(
    style: cobolt_forms::model::ModalOverlayStyle,
) -> Option<egui::Color32> {
    match style {
        // The default: blocked in every way that matters — input refused,
        // focus returned to the child — but painted exactly as designed.
        cobolt_forms::model::ModalOverlayStyle::None => None,
        cobolt_forms::model::ModalOverlayStyle::SemiTransparent => {
            Some(egui::Color32::from_rgba_unmultiplied(60, 60, 64, 64))
        }
        cobolt_forms::model::ModalOverlayStyle::Greyed => {
            Some(egui::Color32::from_rgba_unmultiplied(60, 60, 64, 150))
        }
    }
}

pub fn run(config: FormHostConfig) {
    let title_fallback = config.title_fallback.clone();
    let icon_path = config.icon_path.clone();
    let (app, form) = FormHost::new(config);
    let (fw, fh) = (form.width as f32, form.height as f32);
    let title = window_title(&form.title, title_fallback);
    let (fx_hide_chrome, fx_transparent) = fx_window_flags(&app.fx_entrance);

    let mut viewport = egui::ViewportBuilder::default()
        .with_title(&title)
        // Size the window exactly to the form. A +4 slack here leaves a
        // strip of panel/scrollbar-gutter visible on the right and bottom edges.
        .with_inner_size([fw, fh])
        // `Resizable` false: the borders do not drag (operator, 2026-10-04).
        .with_resizable(form.resizable)
        // ── 037 window chrome from the designed form ──────────────────────
        // Title-bar buttons (R12), chromeless (R15), fullscreen (R14) and the
        // opening WindowState (R13; Maximized here, Minimized via a first-
        // frame viewport command — winit has no pre-minimized builder).
        .with_minimize_button(form.can_minimize)
        .with_maximize_button(form.can_maximize)
        // 038 — while an entrance plays, the window wears no chrome: the title
        // bar would be the one fixed, un-animated element on screen. It is
        // switched back on the frame the animation ends.
        .with_decorations(form.title_visible && !fx_hide_chrome)
        .with_fullscreen(form.full_screen);
    // Window start position — `Custom` is the one variant with a concrete
    // coordinate available before the window exists, so it goes straight
    // into the builder; every screen-relative variant needs the monitor's
    // size, unknown until the window is up (see `pending_start_position`
    // above), and `System` means "do not touch it", exactly like today.
    // 056 R18 — a responsive form's smallest layout is the window's minimum
    // inner size, so the OS grip cannot produce a layout the form cannot
    // honour. A floor only: nothing here sizes the window, and a form that is
    // not responsive keeps no minimum, as before.
    if let Some(l) = cobolt_forms::layout::size_limits_of(&form) {
        let (mw, mh) = l.window_max();
        viewport = viewport.with_min_inner_size([l.min.0, l.min.1]).with_max_inner_size([mw, mh]);
    }
    if form.start_position == cobolt_forms::model::FormStartPosition::Custom {
        viewport = viewport.with_position(egui::pos2(form.x as f32, form.y as f32));
    }
    viewport =
        viewport.with_maximized(form.window_state == cobolt_forms::model::WindowState::Maximized);
    let _ = fx_transparent;
    // See-through when an entrance plays over the desktop OR the form has a
    // `Transparency` of its own: the window must be created carrying alpha,
    // or the property only faded the backdrop toward its base colour and the
    // desktop never showed (property audit, 2026-09-26).
    if app.see_through {
        // The effect plays over the DESKTOP: the surface must carry alpha, and
        // that can only be decided at creation. macOS still draws a drop
        // shadow around a transparent window, which would outline the
        // "invisible" window and give the trick away — and winit only offers
        // that switch at creation too, so it is off for this window's life.
        viewport = viewport.with_transparent(true).with_has_shadow(false);
    }
    // 037 R9 — the MAIN form's TaskbarIcon outranks the project icon; other
    // forms keep the project icon (their windows are taskbar-less once opened
    // via OpenForm*, spec 037 R8).
    // Resolved like every other asset — against the application's folder —
    // so a project-relative path works however the program was launched; a
    // built binary started from elsewhere fell back to the default icon.
    let taskbar_icon_path: Option<PathBuf> =
        if form.main_form && !form.taskbar_icon.trim().is_empty() {
            Some(cobolt_forms::assets::resolve(form.taskbar_icon.trim()))
        } else {
            None
        };
    if let Some(icon) = load_host_icon(taskbar_icon_path.as_deref().or(icon_path.as_deref())) {
        viewport = viewport.with_icon(icon);
    }
    let native_options = crate::native_options(viewport);
    let _ = eframe::run_native(
        &title,
        native_options,
        Box::new(move |cc| {
            // Same base font set the IDE installs: egui's defaults plus the
            // broad-Latin and CJK system fallbacks. Without them this process
            // has Latin only, so katakana (MatrixRain) and CJK captions drew
            // as tofu boxes while the IDE preview showed them correctly.
            cc.egui_ctx
                .set_fonts(cobolt_forms::fonts::base_font_definitions());
            Ok(Box::new(app) as Box<dyn eframe::App>)
        }),
    );
}

impl FormHost {
    /// Construct the host from its config; hands the designed `Form` back so
    /// [`run`] can assemble the OS window from it. Separate from [`run`] so
    /// the parity suite can drive the host headlessly (spec 042 R29).
    pub(crate) fn new(config: FormHostConfig) -> (Self, cobolt_forms::Form) {
        let FormHostConfig {
            form,
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
            form_source,
            child_theme,
            child_interpreter_setup,
            indexed_engine,
            shared_rust_bridge,
            fx_entrance,
            fx_exit,
            fx_restore,
            theme_pack,
            surface_theme,
            icon_path: _,
            title_fallback: _,
            hooks,
            surface,
        } = config;

        // 049 R18 — entrance/exit effects are window effects: a pane-hosted
        // form is simply present. Zeroing the specs here keeps every fx gate
        // below untouched.
        let (fx_entrance, fx_exit) = if surface == Surface::Pane {
            (
                cobolt_forms::window_fx::FxSpec::default(),
                cobolt_forms::window_fx::FxSpec::default(),
            )
        } else {
            (fx_entrance, fx_exit)
        };

        // 049 — in a pane, the SideMenu IS the MenuPane: the shell paints it as
        // chrome outside this host. Rendering the control again inside the
        // ContentPane would put the same sidebar on screen twice, side by side,
        // and `FullHeight` makes that second copy as tall as the whole form.
        // Only its PAINT is dropped — the control keeps its state entry below,
        // so `SelectedItemId` and its event handlers still work.
        // 049 — the column the rail occupies in the DESIGNED form. The shell
        // lays the ContentPane out beside the MenuPane, so the pane's own
        // left edge is already past the rail; a control still carrying its
        // designed x would then be pushed right by the rail's width a SECOND
        // time. The designed width is the one that matters, not the live pane
        // width: Open/Collapsed moves the pane edge, and the form travels with
        // it because it is anchored to the pane, not to the window.
        let (flat, footer_ids, side_dx) = if surface == Surface::Pane {
            pane_layout(flat)
        } else {
            (flat, std::collections::HashSet::new(), 0)
        };

        let glass_style = form.glass_style;
        let form_object = form.name.trim().to_ascii_uppercase();
        // The pane holds the form MINUS the rail's column, so the scroll extent
        // is the content's, not the whole designed form's — otherwise the pane
        // scrolls sideways over a rail-width band of nothing.
        let (fw, fh) = (
            (form.width as f32 - side_dx as f32).max(1.0),
            form.height as f32,
        );

        // R27 — with diagnostics on, say what this window IS before anything
        // runs.
        let diagnostics = crate::diagnostics::frame_diagnostics_enabled();
        if diagnostics {
            let ids: Vec<&str> = flat.iter().map(|c| c.id.as_str()).collect();
            crate::diagnostics::launch_preamble(&form, &ids);
        }

        let (fx_hide_chrome, fx_transparent) = fx_window_flags(&fx_entrance);
        // A see-through form theme (Spatial, spec 083) makes the window
        // see-through too: the desktop behind it shows, blurred.
        // A rounded form too: the desktop must show past its arc, and a window
        // carries alpha only if it is created with it — so a form DESIGNED with
        // a corner radius always gets one, title bar or not, and can round its
        // corners whenever the title bar goes.
        let see_through = fx_transparent
            || (surface != Surface::Pane
                && (form.transparency > 0 || surface_theme.see_through() || form.corner_radius > 0));

        let host = FormHost {
            root: FormBody {
                drawn_reported: false,
                form_name: form.name.clone(),
                title_visible: form.title_visible,
                corner_radius: form.corner_radius,
                see_through_window: false,
                pane_window: None,
                owns_window: surface == Surface::Window,
            last_window_crumb: None,
                footer_ids: footer_ids.clone(),
                theme_pack,
                surface_theme,
                glass_style,
                motion_bound: motion_bindings(&form_object, &form.form_events, &flat),
                controls: flat,
                special_names: form.cobol_structure.special_names.clone(),
                state,
                bg_hex: form.background_color.clone(),
                bg_gradient_enabled: form.background_gradient_enabled,
                bg_gradient_start: form.background_gradient_start_color.clone(),
                bg_gradient_end: form.background_gradient_end_color.clone(),
                bg_gradient_direction: form.background_gradient_direction.clone(),
                transparency: form.transparency.clamp(0, 100) as u8,
                bg_image: form.background_image.clone(),
                bg_mode: form.bg_image_mode,
                use_theme_background: form.use_theme_background,
                modal_overlay_style: form.modal_overlay_style,
                form_size: egui::vec2(fw, fh),
                responsive: ResponsiveSpec::of(&form),
                responsive_off: (!form.lays_out()).then(|| ResponsiveSpec::design(&form)),
                ev_tx,
                input_tx,
                state_rx,
                display_rx,
                pending,
                finished,
                start: std::time::Instant::now(),
                lifecycle_sent: false,
                db_dumped: false,
                form_object,
                anim: cobolt_forms::anim::AnimRuntime::new(fw, fh),
                anim_started: false,
                last_frame: None,
                hovered: std::collections::HashSet::new(),
                parked_timer_clocks: HashMap::new(),
                toolbar_runner: cobolt_forms::toolbar_actions::Runner::default(),
            pending_save_as: Vec::new(),
            pending_os_handoff: Vec::new(),
            os_handoff: OsHandoffChannel::default(),
                action_notice: None,
                last_control_rects: HashMap::new(),
                last_layout: None,
                mirrored: None,
                breakpoint_reported: None,
                font_scale_reported: None,
                snackbars: Default::default(),
                viewer_sessions: Default::default(),
            },
            children: Vec::new(),
            pane: Pane::default(),
            form_req_tx,
            form_source,
            child_theme,
            child_interpreter_setup,
            indexed_engine,
            shared_rust_bridge,
            surface,
            footer_ids,
            visuals_set: false,
            quit_sent: false,
            diagnostics,
            start_minimized: form.window_state == cobolt_forms::model::WindowState::Minimized,
            supervisor: cobolt_runtime::form_host::FormSupervisor::new(&form.name, &form.name),
            form_req_rx,
            closed: ClosedFanout::new(closed_tx),
            fullscreen_actual: form.full_screen,
            root_min_inner: cobolt_forms::layout::size_limits_of(&form).map(|l| {
                let (mw, mh) = l.window_max();
                (egui::vec2(l.min.0, l.min.1), egui::vec2(mw, mh))
            }),
            fx_entrance,
            fx_exit,
            fx_restore,
            fx_entrance_start: None,
            fx_entrance_done: !fx_entrance.is_active(),
            fx_exit_start: None,
            fx_seed: {
                // Deterministic per window: name + size (stable across restarts).
                let mut h = 0x811C_9DC5_u32;
                for b in form.name.bytes() {
                    h = (h ^ b as u32).wrapping_mul(0x0100_0193);
                }
                h ^ form.width ^ form.height.rotate_left(16)
            },
            minimized_actual: form.window_state == cobolt_forms::model::WindowState::Minimized,
            maximized_actual: form.window_state == cobolt_forms::model::WindowState::Maximized,
            // Seeded so the FIRST frame is never reported as a change: a window
            // opens focused, and its theme/DPI/geometry are whatever the first
            // frame observes rather than a transition into them.
            focused_actual: true,
            system_theme_actual: None,
            dpi_actual: None,
            window_size_actual: None,
            window_size_reported: None,
            rail_dx: side_dx as f32,
            window_pos_actual: None,
            resize_pending: false,
            move_pending: false,
            drag_hovering: false,
            scrolling: false,
            pointer_inside: false,
            pointer_down: false,
            fx_transparent,
            see_through,
            fx_chrome_pending: fx_hide_chrome && form.title_visible,
            fx_chrome_restore: None,
            fx_chrome_hidden_for_exit: false,
            // Window start position: the eight edge/corner positions and
            // Center need the monitor's actual size, which the builder cannot
            // know before the window exists — set on the first frame. `System`
            // (do nothing) and `Custom` (already in the viewport builder)
            // need no first-frame command at all.
            pending_start_position: form
                .start_position
                .is_screen_relative()
                .then_some(form.start_position),
            // `ScreenFill`: the share of the screen the window opens at, within
            // the form's own size limits — applied on the first frame the
            // monitor's size is known, before the start position.
            pending_screen_fill: (surface == Surface::Window && form.screen_fill > 0).then(|| {
                let limits = cobolt_forms::layout::size_limits_of(&form);
                let min = limits.as_ref().map_or((0.0, 0.0), |l| l.min);
                let max = limits.as_ref().map_or(
                    (cobolt_forms::model::FORM_MAX_SIZE as f32, cobolt_forms::model::FORM_MAX_SIZE as f32),
                    |l| l.window_max(),
                );
                (form.screen_fill, min, max)
            }),
            group_fit_seen: None,
            group_fit_started: None,
            group_fit_zoom: 1.0,
            hooks,
            last_pane_backdrop_rect: None,
            last_pane_backdrop_fill: None,
            last_content_scroll: egui::Vec2::ZERO,
            pending_menu_pane: None,
        };
        (host, form)
    }
}

// ── FormHost ──────────────────────────────────────────────────────────────────

/// 051 — everything that belongs to ONE hosted form: its design, live state,
/// channels, animation clocks and per-form lifecycle one-shots. The root
/// window holds one; each child window and each pane occupant holds its own,
/// all rendered through the same frame path — one renderer, N forms.
/// The lookup key one control-and-event pair gets in [`FormBody::motion_bound`].
///
/// A COBOL word reaches the runtime upper-cased and an event name is written
/// however the designer typed it, so both halves are folded before comparing.
pub(crate) fn motion_key(ctrl: &str, event: &str) -> String {
    format!(
        "{}\u{1}{}",
        ctrl.trim().to_ascii_uppercase(),
        event.trim().to_ascii_lowercase()
    )
}

/// Every pointer-motion event this form has a handler for — the form's own
/// bindings and each control's.
///
/// A binding with an EMPTY body still counts: the developer wrote the handler,
/// and an empty one is a handler they have not filled in yet, not permission to
/// throw their events away.
pub(crate) fn motion_bindings(
    form_object: &str,
    form_events: &[cobolt_forms::model::EventBinding],
    controls: &[cobolt_forms::Control],
) -> std::collections::HashSet<String> {
    let mut out = std::collections::HashSet::new();
    for b in form_events {
        if cobolt_forms::diagnostics::is_motion_event(&b.event) {
            out.insert(motion_key(form_object, &b.event));
        }
    }
    for c in controls {
        for b in &c.events {
            if cobolt_forms::diagnostics::is_motion_event(&b.event) {
                out.insert(motion_key(&c.id, &b.event));
            }
        }
    }
    out
}

/// Both ends of the channel a finished handoff reports back on.
///
/// One field rather than two, so a constructor cannot create a sender whose
/// receiver was never kept — which is the whole of what a split pair invites.
pub(crate) struct OsHandoffChannel {
    pub tx: mpsc::Sender<(String, crate::os_handoff::Handoff, crate::os_handoff::Outcome)>,
    pub rx: mpsc::Receiver<(String, crate::os_handoff::Handoff, crate::os_handoff::Outcome)>,
}

impl Default for OsHandoffChannel {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        Self { tx, rx }
    }
}

pub(crate) struct FormBody {
    pub(crate) form_name: String,
    /// The window shows its title bar — the form's `TitleVisible`, kept
    /// current by `SetTitleVisible`. Without one the window moves by its face
    /// (operator, 2026-10-03; see `Backdrop::draggable`).
    pub(crate) title_visible: bool,
    /// The form's `CornerRadius`, kept current by `me::CornerRadius`. It
    /// rounds the window only while the title bar is off (see
    /// `Backdrop::rounding`), and only a window created see-through can show
    /// it — `see_through_window`, or the root host's `see_through`.
    pub(crate) corner_radius: u32,
    /// This CHILD window was created see-through because its form is rounded:
    /// its panel then fills nothing, and the engine's rounded backdrop is the
    /// only paint, so the desktop shows past the arc.
    pub(crate) see_through_window: bool,
    /// The rounded window this body is drawn INTO when it does not own one —
    /// a shell's ContentPane, set fresh each frame by whoever draws it. The
    /// pane is one piece of that window, so its backdrop and top-level
    /// controls round only the window corners they reach.
    pub(crate) pane_window: Option<cobolt_forms::paint::ContainerClip>,
    /// This body IS a window: the root of a `Surface::Window` host, or a
    /// child window. A ContentPane occupant draws inside the shell's window
    /// and must never move it.
    pub(crate) owns_window: bool,
    /// 049/051 — where this body's OWN breadcrumb strip put its pieces last
    /// frame, for a form running in a stand-alone WINDOW. `None` for a body
    /// with no SideMenu, and for the ContentPane occupant, whose strip is the
    /// shell's. See [`Self::window_crumb_chrome`].
    pub(crate) last_window_crumb: Option<cobolt_forms::breadcrumb::BreadcrumbLayout>,
    /// `drawn_rects` has already been reported for this body — it is printed
    /// ONCE, on the first frame that actually placed controls, not per frame.
    pub(crate) drawn_reported: bool,
    /// 049 — ids the CONTENT pass must not draw because the RAIL draws them:
    /// the SideMenu footer Panel and whatever was dropped into it. Empty in a
    /// window host, where the rail is an ordinary control.
    pub(crate) footer_ids: std::collections::HashSet<String>,
    /// Resolved asset-pack theme (None = built-in Liquid Glass) + the form's
    /// glass style — pushed into the egui context so the unified painter reads
    /// the same theme state as under the IDE (spec 017 parity).
    pub(crate) theme_pack: Option<Arc<cobolt_forms::theme_pack::ThemePack>>,
    pub(crate) surface_theme: std::sync::Arc<dyn cobolt_forms::surface_theme::SurfaceTheme>,
    pub(crate) glass_style: cobolt_forms::model::GlassStyle,
    pub(crate) controls: Vec<cobolt_forms::Control>,
    /// Which per-frame POINTER-MOTION events actually have a handler behind
    /// them, keyed `"CTRL-ID-UPPERCASED\u{1}eventlowercased"`.
    ///
    /// Every other event is a discrete act — a click, a key, a close — and one
    /// per act is one event. `onMouseMove`/`onPointerMove` are not: they fire
    /// on every frame the pointer moves, two per frame, ~120 a second, and
    /// [`Self::send_event`] used to queue all of them whether or not the form
    /// had ever bound one. The interpreter retires exactly ONE event per
    /// `COBOL-WAIT-EVENT`, so the queue only stays short while the interpreter
    /// is free to drain it. A synchronous `AgentObject::Ask` is precisely when
    /// it is not: the thread sits inside the HTTP call for the whole answer,
    /// consuming nothing, while the pointer keeps writing. Raising
    /// `MaximumTokens` from 400 to 8192 turned a two-second call into a
    /// half-minute one and the backlog with it — the form answered the first
    /// few questions and then never caught up, and Timer ticks stopped too
    /// (they are coalesced away whenever `pending` is this far behind)
    /// (operator, 2026-09-07: "it works for the first very few questions, then
    /// it stops").
    pub(crate) motion_bound: std::collections::HashSet<String>,
    /// The form's `SPECIAL-NAMES` paragraph, verbatim. A control's `Picture`
    /// takes its decimal separator and currency character from here, so the
    /// running form reads `DECIMAL-POINT IS COMMA` exactly as the generated
    /// program does. Carried as text rather than as the whole `Form` because
    /// this is the only part of it the body needs.
    pub(crate) special_names: String,
    pub(crate) state: HashMap<String, CtrlState>,
    pub(crate) bg_hex: String,
    pub(crate) bg_gradient_enabled: bool,
    pub(crate) bg_gradient_start: String,
    pub(crate) bg_gradient_end: String,
    pub(crate) bg_gradient_direction: String,
    pub(crate) transparency: u8,
    pub(crate) bg_image: String,
    pub(crate) bg_mode: cobolt_forms::model::BgImageMode,
    /// The form's `UseThemeBackground` opt-in — the pack's background art
    /// replaces the form's own image when the active theme provides one.
    pub(crate) use_theme_background: bool,
    /// 051 R19/R28 — how THIS form's own face paints while blocked by a
    /// modal child of its own (`child_frame`'s `blocked` overlay).
    pub(crate) modal_overlay_style: cobolt_forms::model::ModalOverlayStyle,
    pub(crate) form_size: egui::Vec2,
    /// Spec 056 — `Some` when the form is responsive: its layout properties
    /// and breakpoint table. Every surface lays such a form out for its own
    /// size before rendering it (R23); `None` renders exactly as before (R3).
    pub(crate) responsive: Option<ResponsiveSpec>,
    /// Spec 056 R84 — the responsive design of a form running with it switched
    /// off, kept so `me::Responsive = 1` can switch it on.
    pub(crate) responsive_off: Option<ResponsiveSpec>,
    pub(crate) ev_tx: mpsc::Sender<FormEvent>,
    pub(crate) input_tx: mpsc::Sender<StateUpdate>,
    pub(crate) state_rx: mpsc::Receiver<StateUpdate>,
    pub(crate) display_rx: mpsc::Receiver<String>,
    /// Events queued for the interpreter — lets the render Timer arm coalesce
    /// ticks against a backlog and drives the fast-repaint branch below.
    pub(crate) pending: Arc<AtomicUsize>,
    /// Set by the interpreter thread when the program ends (STOP RUN or error).
    pub(crate) finished: Arc<AtomicBool>,
    /// When the form appeared. Input is ignored for a short warm-up so a click
    /// in progress as it appears can't fire a phantom event.
    pub(crate) start: std::time::Instant,
    pub(crate) lifecycle_sent: bool,
    /// One-shot guard for the `COBOLT_DATABIND_TRACE` render-side dump.
    pub(crate) db_dumped: bool,
    /// The form's object name (UPPER) — receiver of form-level events.
    pub(crate) form_object: String,
    pub(crate) anim: cobolt_forms::anim::AnimRuntime,
    pub(crate) anim_started: bool,
    pub(crate) last_frame: Option<std::time::Instant>,
    /// Control ids under the pointer last frame (animation hover triggers).
    pub(crate) hovered: std::collections::HashSet<String>,
    /// 051 Q2 (operator ruling) — per-Timer clocks used while this form is
    /// PARKED (off-pane): render-driven timers stop with the rendering, so
    /// the host ticks these instead and timer handlers keep running.
    pub(crate) parked_timer_clocks: HashMap<String, std::time::Instant>,
    /// Carries out a toolbar button's PLATFORM action, and finishes the
    /// two-frame window captures.
    ///
    /// Per FORM, not per host: a capture is of one window, so a child window's
    /// screenshot is its own. It used to live on the host, which is part of why
    /// only the root form ever ran a toolbar action at all.
    pub(crate) toolbar_runner: cobolt_forms::toolbar_actions::Runner,
    /// Save As dialogs asked for while draining the state channel, which has no
    /// `egui::Context` — opened by `start_pending_save_as` on the same frame.
    pub(crate) pending_save_as: Vec<(String, crate::file_dialog::DialogSpec)>,
    /// R19/R20 handoffs asked for during the same drain, started on the same
    /// frame by `drive_viewer_os_handoffs`.
    pub(crate) pending_os_handoff: Vec<(String, crate::os_handoff::Handoff, PathBuf)>,
    /// Where a finished handoff reports back from its thread.
    pub(crate) os_handoff: OsHandoffChannel,
    /// The latest platform-action outcome, shown briefly in the form window
    /// (message, is_error, egui time it appeared). A Failed print or an
    /// empty-clipboard paste used to go only to stderr — to the operator that
    /// read as "the button does nothing" (2026-08-23).
    pub(crate) action_notice: Option<(String, bool, f64)>,
    /// Where the last rendered frame actually PUT each control, in screen
    /// coordinates — the engine's own `RenderOutput::control_rects`, kept.
    ///
    /// A control that is painted but lands outside the surface it was drawn
    /// into is indistinguishable, from the outside, from a control that was
    /// never painted at all: both are simply not on screen. Recording the
    /// rects is what tells those two apart without a debugger, and it is what
    /// the ContentPane placement test asserts against (operator, 2026-08-31:
    /// radios missing from an embedded form).
    pub(crate) last_control_rects: HashMap<String, egui::Rect>,
    /// Spec 056 — the layout this body was last drawn with (responsive forms
    /// only): where a COBOL geometry write is read back from, and what it is
    /// inverted through (R37, R38).
    pub(crate) last_layout: Option<cobolt_forms::layout::LayoutOutput>,
    /// Spec 056 R37 — the rectangle the program was last told for each
    /// control (its seeded design until the first layout), so only a change
    /// is mirrored.
    pub(crate) mirrored: Option<HashMap<String, cobolt_forms::model::Rect>>,
    /// Spec 056 R46/R47 — the breakpoint and font factor the program was last
    /// told; `None` until the first layout, which raises no event.
    pub(crate) breakpoint_reported: Option<String>,
    pub(crate) font_scale_reported: Option<f32>,
    /// 055 — this surface's live notifications. One stack per FormBody, which
    /// is what "the stack belongs to the surface" means (spec Q1/Q2): a child
    /// form's messages stack in that child, and navigating away disposes them
    /// rather than carrying a message about screen A onto screen B.
    pub(crate) snackbars: crate::snackbar_stack::SnackbarStack,
    /// 058 — one live session per Viewer control on this surface, keyed by
    /// control id: its decode thread, bounded page cache, view state and
    /// (for Streamed layout) conversation history. Lazily populated — a
    /// Viewer with no document open yet has no entry here at all.
    pub(crate) viewer_sessions: HashMap<String, crate::viewer_session::ViewerSession>,
}

/// The id space the SideMenu footer fragment renders in.
///
/// A CONSTANT, deliberately — not `ui.id()`. These ids key focus, scroll
/// offsets and combo state from one frame to the next, so an id that moved when
/// the `Ui` tree shifted would drop the operator's focus mid-keystroke. The
/// same value goes to `render_form_scoped` and to every `control_widget_id`
/// lookup for that surface; they must not drift apart.
pub(crate) fn footer_id_scope() -> egui::Id {
    egui::Id::new("cobolt-sidemenu-footer")
}

impl FormBody {
    /// Spec 058 R5/R5.1 — give every Viewer on this form its own decode
    /// thread, and collect whatever those threads have finished.
    ///
    /// Called once per frame, **before** the render: it asks (never waits),
    /// drains (never blocks) and returns. The expensive part — indexing a
    /// document that may be gigabytes — happens on the control's own thread,
    /// which is the whole of R5.1 and the reason `spec.md`'s headline user
    /// story ("open a 2 GB log and jump to its end immediately") is
    /// achievable at all.
    ///
    /// A session is created per control and kept for as long as the control
    /// exists; sessions for controls that have gone are dropped, which closes
    /// their jobs channel and lets the worker exit on its own — the
    /// detach-don't-join shape, never a `.join()` on this thread.
    pub(crate) fn tick_viewers(&mut self, ctx: &egui::Context) {
        use cobolt_forms::ControlType;

        let viewers: Vec<cobolt_forms::Control> = self
            .controls
            .iter()
            .filter(|c| c.control_type == ControlType::Viewer)
            .map(|c| match self.state.keys().find(|k| k.eq_ignore_ascii_case(&c.id)) {
                Some(k) => cobolt_forms::render::merge_props(c, self.state[k].props.iter()),
                None => c.clone(),
            })
            .collect();

        // Controls that are gone take their thread with them.
        self.viewer_sessions.retain(|id, _| viewers.iter().any(|c| &c.id == id));
        if viewers.is_empty() {
            return;
        }

        let mut wanted_repaint = false;
        for ctrl in &viewers {
            let session = self
                .viewer_sessions
                .entry(ctrl.id.clone())
                .or_insert_with(|| crate::viewer_session::ViewerSession::new(ctrl.id.clone()));

            let mode = cobolt_forms::viewer::SplitMode::from_str(
                &ctrl.get_prop("SplitMode").map(|v| v.as_str().to_owned()).unwrap_or_default(),
            );
            for view in 0..mode.view_count() {
                let source = cobolt_forms::viewer::view_prop("Source", view);
                let mut path = ctrl
                    .get_prop(&source)
                    .map(|v| v.as_str().trim().to_owned())
                    .unwrap_or_default();
                if view == 0 && path.is_empty() {
                    path = ctrl.get_prop("Source").map(|v| v.as_str().trim().to_owned()).unwrap_or_default();
                }
                if path.is_empty() {
                    continue;
                }
                let page = cobolt_forms::viewer::view_prop("Page", view);
                let page = ctrl.get_prop(&page).map(|v| v.as_i64()).unwrap_or(1).max(1) as usize - 1;
                if session.document(&path).is_none() {
                    // Asked for but not answered yet: keep the frames coming
                    // so the document appears the moment it is ready.
                    wanted_repaint = true;
                }
                session.request(&path, page);
            }
            session.drain_completed();
        }
        if wanted_repaint {
            ctx.request_repaint();
        }
    }

    /// Everything this form's decode threads have finished, by source path.
    pub(crate) fn viewer_documents(
        &self,
    ) -> std::collections::HashMap<String, std::sync::Arc<cobolt_forms::paint::ViewerDocument>> {
        let mut out = std::collections::HashMap::new();
        for session in self.viewer_sessions.values() {
            for (source, doc) in session.ready_documents() {
                out.insert(source, doc);
            }
        }
        out
    }


    /// Which page each `TabControl` is showing RIGHT NOW.
    ///
    /// `containers::is_visible` decides a control's visibility from this map,
    /// and falls back to the DESIGNED `SelectedTab` when a TabControl is not in
    /// it. Every host path used to hand it `ActiveTabs::default()` — an empty
    /// map, rebuilt empty every frame — so the fallback was the only thing that
    /// ever answered, and the designed page was the only page a running form
    /// could show. Clicking a tab wrote `SelectedTab` into the live state and
    /// then nothing happened, while the IDE's Preview (which builds this map
    /// from its own live values) worked perfectly (operator, 2026-09-09:
    /// "TabControl: Clicking in a Run form does not change the page. Preview
    /// works fine").
    ///
    /// Built here rather than in each of the three frame paths, because a form
    /// behaviour has to reach `rcrun run-form`, an embedded child form AND the
    /// compiled binary.
    pub(crate) fn active_tabs(&self) -> cobolt_forms::containers::ActiveTabs {
        self.controls
            .iter()
            .filter(|c| c.control_type == cobolt_forms::ControlType::TabControl)
            .filter_map(|c| {
                let live = self
                    .state
                    .keys()
                    .find(|k| k.eq_ignore_ascii_case(&c.id))
                    .and_then(|k| self.state.get(k))
                    .and_then(|st| {
                        st.props
                            .iter()
                            .find(|(k, _)| k.eq_ignore_ascii_case("SelectedTab"))
                            .map(|(_, v)| v.clone())
                    });
                live.and_then(|v| v.trim().parse::<u32>().ok())
                    .map(|tab| (c.id.clone(), tab))
            })
            .collect()
    }

    /// Draw the SideMenu's footer Panel — and whatever the developer dropped
    /// into it — inside the rail's own footer band, and forward what the
    /// operator does there to the interpreter.
    ///
    /// The footer Panel is the developer's: they drop controls into it in the
    /// designer and style it through the ordinary inspector. In a SHELL the
    /// rail is chrome drawn outside the ContentPane, so those controls have no
    /// business in the pane's list — left there they were slid over with the
    /// rest of the form and clamped to the pane's left edge, surfacing BESIDE
    /// the rail instead of on it (operator, 2026-08-22).
    ///
    /// `band` is the live footer row from `sidebar::layout`, so the panel
    /// follows the rail's height, the operator's `FooterHeight` and a collapsed
    /// rail without any of those knowing about this. The subtree is REBASED on
    /// the panel's designed origin, which is what keeps a control's position
    /// inside the footer exactly what the designer showed.
    ///
    /// Nothing here is a second copy of the render: it is the same engine, the
    /// same live state and the same event forwarding as the content pass —
    /// only the `Ui` it draws into is different.
    pub(crate) fn draw_side_menu_footer(
        &mut self,
        ui: &mut egui::Ui,
        band: egui::Rect,
        behind: egui::Color32,
        blocked: bool,
    ) {
        if self.footer_ids.is_empty() || band.width() < 1.0 || band.height() < 1.0 {
            return;
        }
        // The panel's designed origin — everything in the band is placed
        // relative to it.
        let Some(origin) = self
            .controls
            .iter()
            .find(|c| c.is_side_menu_footer() && self.footer_ids.contains(&c.id))
            .map(|c| (c.rect.x, c.rect.y))
        else {
            return;
        };
        let subtree: Vec<cobolt_forms::Control> = self
            .controls
            .iter()
            .filter(|c| self.footer_ids.contains(&c.id))
            .map(|c| {
                let mut c = c.clone();
                c.rect.x -= origin.0;
                c.rect.y -= origin.1;
                c
            })
            .collect();

        // `hidden: None` — this IS the pass that owns them.
        let st = LiveState {
            state: &self.state,
            anim: &self.anim,
            hidden: None,
            viewer_docs: None,
                special_names: &self.special_names,
        };
        let active_tabs = self.active_tabs();
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(band));
        child.set_clip_rect(band.intersect(ui.clip_rect()));
        let input = cobolt_forms::render::RenderInput {
            controls: &subtree,
            state: &st,
            form_size: band.size(),
            glass: true,
            mode: cobolt_forms::render::RenderMode::Interactive,
            active_tabs: &active_tabs,
            // The RAIL painted this band. This pass adds the Panel and its
            // contents ON it and paints no background of its own — a default
            // Backdrop paints the form's own navy, which is what turned a
            // 100 %-transparent footer Panel into a black block over the rail
            // (operator, 2026-08-22). `behind` is the rail's own fill, so a
            // translucent Panel still has something to resolve against.
            backdrop: cobolt_forms::render::Backdrop::behind(behind),
        };
        // Focus BEFORE the footer's widgets see the click — same rule as the
        // main frame paths (the press surrenders the field's focus).
        let pre_focus = ui.ctx().memory(|m| m.focused());
        // …and the footer renders in its OWN id space.
        //
        // This is a SECOND form surface inside one egui viewport: the pane
        // beside it holds another form entirely, and both derive widget ids
        // from control ids. A `TextBox-1` in this footer and a `TextBox-1` in
        // the pane occupant asked egui for the same id, and it painted "Second
        // use of widget ID" over the pane's — which had done nothing wrong
        // (operator, 2026-09-02, the Ferris Says sample). `push_id` cannot
        // separate them: the engine's control ids are absolute by design, so
        // the id space has to be passed in.
        let out = child
            .push_id("sidemenu-footer", |ui| {
                cobolt_forms::render::render_form_scoped(ui, &input, None, footer_id_scope())
            })
            .inner;

        // A toolbar button (or FileDropZone) in the footer is as real as one
        // on the form: its platform actions used to be dropped here — only the
        // COBOL event was forwarded, so print/copy/share in a footer did
        // nothing, silently (operator, 2026-08-23).
        let ctx = ui.ctx().clone();
        self.run_platform_requests(
            &ctx,
            &out.file_picker_requests,
            &out.csv_export_requests,
            &out.toolbar_actions,
            pre_focus,
            Some(footer_id_scope()),
        );
        self.forward_interaction(&out.prop_updates, out.events, blocked);
    }

    // ── Snackbar (spec 055) ─────────────────────────────────────────────────

    /// The pseudo-properties `Show()` and `DismissAll()` arrive as.
    ///
    /// The interpreter cannot call into this crate, so a control method reaches
    /// the host the way `PlayAnimation` already does: `obj_set` writes a
    /// pseudo-property, the `StateUpdate` crosses the channel, and the host acts
    /// on it here. No new channel, no new message type.
    ///
    /// Returns true when the write was a Snackbar command and must NOT be
    /// stored as ordinary control state.
    /// The key a Viewer's Save As dialog is tracked under.
    pub(crate) fn viewer_save_as_key(ctrl_id: &str) -> String {
        format!("viewersaveas:{ctrl_id}")
    }

    /// Spec 058 R18.1 — the runtime asking this crate for a destination.
    ///
    /// The division of labour is the one `file_picker_requests` and
    /// `csv_export_requests` already draw, one step further along: the RUNTIME
    /// owns the document, so it owns the saving and the suggested filename;
    /// this crate owns the dialog and nothing else; and neither can call the
    /// other, so they speak through the state channel.
    ///
    /// `value` is the suggested filename, computed where the document actually
    /// is. The answer goes back as `_SaveAsAnswer` — a path, or EMPTY for a
    /// dialog the operator dismissed, which is `onSaveCancelled` and not an
    /// error.
    ///
    /// Returns true when the write was that request and must NOT be stored as
    /// ordinary control state.
    pub(crate) fn viewer_save_as_request(
        &mut self,
        ctrl_id: &str,
        prop: &str,
        value: &str,
    ) -> bool {
        if !prop.eq_ignore_ascii_case("_SaveAsRequest") {
            return false;
        }
        // The runtime CLEARS the request once it has been answered, and that
        // clear crosses the channel like any other write. Opening a dialog for
        // it would raise a second one the moment the first was answered.
        let suggested = value.trim();
        if suggested.is_empty() {
            return true;
        }
        let suggested = Path::new(suggested);
        let mut spec = crate::file_dialog::DialogSpec::save();
        // `SaveAsPdf()` proposes a `.pdf`: the panel offers that type.
        if suggested
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("pdf"))
        {
            spec = spec.filter("PDF", &["pdf"]);
        }
        if let Some(name) = suggested.file_name().and_then(|n| n.to_str()) {
            spec = spec.file_name(name);
        }
        if let Some(dir) = suggested.parent().filter(|p| !p.as_os_str().is_empty()) {
            spec = spec.directory(dir);
        }
        // Queued rather than opened here: this runs while draining the state
        // channel, which has no `egui::Context` to hand — and a dialog needs
        // one to wake the frame its answer arrives on.
        self.pending_save_as.push((ctrl_id.to_owned(), spec));
        true
    }

    /// Open whatever Save As dialogs the state drain queued this frame.
    pub(crate) fn start_pending_save_as(&mut self, ctx: &egui::Context) {
        for (id, spec) in std::mem::take(&mut self.pending_save_as) {
            crate::file_dialog::begin(ctx, &Self::viewer_save_as_key(&id), spec);
        }
    }

    /// Spec 058 R19/R20 — the runtime asking this crate to hand a document to
    /// the platform.
    ///
    /// Same division of labour as the Save As panel above, one step further
    /// along: the runtime owns the document and has already resolved it to a
    /// path (writing one out for a `LoadBytes` document, because a platform
    /// takes a file), and this crate owns the platform and nothing else.
    ///
    /// Returns true when the write was one of those requests and must NOT be
    /// stored as ordinary control state.
    pub(crate) fn viewer_os_request(&mut self, ctrl_id: &str, prop: &str, value: &str) -> bool {
        use crate::os_handoff::Handoff;
        let what = if prop.eq_ignore_ascii_case(Handoff::Print.request_prop()) {
            Handoff::Print
        } else if prop.eq_ignore_ascii_case(Handoff::Share.request_prop()) {
            Handoff::Share
        } else {
            return false;
        };
        // The runtime CLEARS the request once it has been answered, and that
        // clear crosses the channel like any other write.
        let path = value.trim();
        if path.is_empty() {
            return true;
        }
        self.pending_os_handoff
            .push((ctrl_id.to_owned(), what, PathBuf::from(path)));
        true
    }

    /// Start whatever handoffs the state drain queued, and collect the answers
    /// to ones started earlier.
    ///
    /// Both here, beside the drain that raises them, because a print spooler
    /// or a desktop opener can take seconds: the handoff is started on one
    /// frame and answered on some later one, exactly as a native dialog is.
    pub(crate) fn drive_viewer_os_handoffs(&mut self, ctx: &egui::Context) {
        for (id, what, path) in std::mem::take(&mut self.pending_os_handoff) {
            crate::os_handoff::hand_off_async(
                what,
                path,
                id,
                self.os_handoff.tx.clone(),
                ctx.clone(),
            );
        }
        while let Ok((id, what, outcome)) = self.os_handoff.rx.try_recv() {
            // The reason FIRST, then the answer: the runtime raises the
            // Cancelled event when it reads the answer, and a handler that
            // reads `LastError` must find it already there.
            if !outcome.accepted && !outcome.reason.is_empty() {
                self.state_entry_mut(&id).set("LastError", outcome.reason.clone());
                let _ = self
                    .input_tx
                    .send(StateUpdate::new(id.clone(), "LastError", outcome.reason));
            }
            let answer = if outcome.accepted { "1" } else { "" };
            self.state_entry_mut(&id).set(what.answer_prop(), answer.to_owned());
            let _ = self
                .input_tx
                .send(StateUpdate::new(id.clone(), what.answer_prop(), answer));
            self.wake_for_input(&id);
        }
    }

    /// Hand back whatever the operator did with a Save As dialog.
    ///
    /// Polled every frame, exactly as the DataGrid's CSV panel is: a native
    /// dialog is non-blocking here (spec 042 R25), so its answer arrives on
    /// some later frame and has to be collected rather than awaited.
    pub(crate) fn collect_viewer_save_as(&mut self) {
        let viewers: Vec<String> = self
            .controls
            .iter()
            .filter(|c| matches!(c.control_type, cobolt_forms::ControlType::Viewer))
            .map(|c| c.id.clone())
            .collect();
        for id in viewers {
            let Some(answer) = crate::file_dialog::take(&Self::viewer_save_as_key(&id)) else {
                continue;
            };
            // A dismissed dialog answers with the EMPTY string rather than not
            // answering at all: the runtime has to hear about it, or the
            // control waits for a save that is never coming and no
            // `onSaveCancelled` is ever raised.
            let path = answer
                .map(|p| p.display().to_string())
                .unwrap_or_default();
            self.state_entry_mut(&id).set("_SaveAsAnswer", path.clone());
            let _ = self
                .input_tx
                .send(StateUpdate::new(id.clone(), "_SaveAsAnswer", path));
            self.wake_for_input(&id);
        }
    }

    /// Wake the interpreter to read an answer just written on the input
    /// channel — see `FormSupervisor::INPUT_WAKE_EVENT`.
    pub(crate) fn wake_for_input(&mut self, ctrl_id: &str) {
        self.send_event(FormEvent::new(
            ctrl_id,
            cobolt_runtime::form_host::FormSupervisor::INPUT_WAKE_EVENT,
        ));
    }

    pub(crate) fn snackbar_command(&mut self, ctrl_id: &str, prop: &str, value: &str) -> bool {
        if prop.eq_ignore_ascii_case("_ShowSnackbar") {
            // Mint from the control's CURRENT property values (D2). The state
            // map carries the live values a handler has been writing, so the
            // snapshot is what the developer set *by the time Show() ran*.
            let Some(ctrl) = self.snackbar_template(ctrl_id) else {
                return true;
            };
            let (visual, diag) = cobolt_forms::snackbar::mint(&ctrl);
            if let Some(d) = diag {
                // Never a silent truncation (spec Q5). At run time the designer
                // warning is not visible, so it goes to the diagnostics trace.
                cobolt_forms::diagnostics::trace_display(&format!(
                    "[snackbar] {ctrl_id}: {d:?} — the first {} are shown",
                    cobolt_forms::snackbar::MAX_BUTTONS
                ));
            }
            self.snackbars.raise(ctrl_id, visual, std::time::Instant::now());
            let _ = value;
            return true;
        }
        if prop.eq_ignore_ascii_case("_DismissAllSnackbar") {
            self.snackbars.dismiss_all(ctrl_id);
            return true;
        }
        // An unhandled COBOL exception, with nobody bound to
        // `onUnhandledException`. The form CONTINUES (operator ruling,
        // 2026-09-06) and the operator is told, so a failure is never silent.
        //
        // Minted from a SYNTHETIC template rather than a designed control:
        // a form that has not thought about errors is exactly the one with no
        // Snackbar on it, and that must not be the reason the message is lost.
        // Critical, and it never expires — `Timeout 0` — so it waits for the
        // close button rather than vanishing while the operator is elsewhere.
        if prop.eq_ignore_ascii_case("_CriticalException") {
            let mut template = cobolt_forms::Control::new(
                ctrl_id,
                cobolt_forms::ControlType::Snackbar,
                0,
                0,
            );
            template.set_prop(
                "Text",
                cobolt_forms::PropValue::String(value.to_owned()),
            );
            template.set_prop(
                "Category",
                cobolt_forms::PropValue::String("Critical".into()),
            );
            // Never expires: it waits for the operator rather than vanishing
            // while they are elsewhere. The ✕ that dismisses it is BUILT IN to
            // every notification — not a property, and not a `Buttons` entry —
            // so the operator's "with an x to close it" needs nothing here.
            template.set_prop("Timeout", cobolt_forms::PropValue::Int(0));
            let (mut visual, _) = cobolt_forms::snackbar::mint(&template);
            // The whole message, never ellipsized: its end says what to do
            // (operator, 2026-10-04 — a refused toolbar call's list of what is
            // allowed was cut off).
            visual.size = cobolt_forms::snackbar::SnackSize::Whole;
            self.snackbars
                .raise(ctrl_id, visual, std::time::Instant::now());
            return true;
        }
        false
    }

    /// The designed Snackbar control, with every live property write applied —
    /// the template as it stands right now.
    fn snackbar_template(&self, ctrl_id: &str) -> Option<cobolt_forms::Control> {
        let base = self
            .controls
            .iter()
            .find(|c| c.id.eq_ignore_ascii_case(ctrl_id))?;
        let mut ctrl = base.clone();
        if let Some((_, st)) = self.state.iter().find(|(k, _)| k.eq_ignore_ascii_case(ctrl_id)) {
            for (k, v) in &st.props {
                ctrl.set_prop(k.clone(), cobolt_forms::model::PropValue::String(v.clone()));
            }
        }
        Some(ctrl)
    }

    /// Tick the stack, lay it out on `surface` and paint it — then forward
    /// everything it reported as COBOL events.
    ///
    /// `surface` is the pane this body was drawn into, **origin included** (D3
    /// / R16): the ContentPane for an Embedded form, the viewport for a
    /// standalone one. That is the rect `child_frame` already computed for the
    /// backdrop, so the two cannot disagree about where this form lives.
    ///
    /// Nothing here can change the surface's size — the rects are computed
    /// *inside* it and painted on the caller's painter (R26/AC13).
    pub(crate) fn draw_snackbars(&mut self, ui: &mut egui::Ui, surface: egui::Rect) {
        if self.snackbars.is_empty() {
            return;
        }
        let now = std::time::Instant::now();
        let pointer = ui
            .ctx()
            .pointer_latest_pos()
            .map(|p| (p.x, p.y));
        self.snackbars.tick(now, pointer);
        // The same pointer the pause-on-hover tick uses, in the form the painter
        // wants: a button has no `Response` to read a hover off, so the state has
        // to be handed to `draw_snackbar` explicitly.
        let snack_pointer = cobolt_forms::paint::SnackPointer {
            pos: ui.ctx().pointer_latest_pos(),
            held: ui.input(|i| i.pointer.primary_down()),
        };

        let painter = ui.painter().clone();
        let surf = cobolt_forms::model::Rect::new(
            surface.min.x.round() as i32,
            surface.min.y.round() as i32,
            surface.width().round() as i32,
            surface.height().round() as i32,
        );
        // Measure through the same painter that will draw it, so the width a
        // notification is GIVEN is the width its text was measured against.
        let measure = |v: &cobolt_forms::snackbar::SnackVisual| {
            let font = egui::FontId::proportional(v.font_size);
            let text_w = |s: &str| -> f32 {
                if s.is_empty() {
                    0.0
                } else {
                    painter
                        .layout_no_wrap(s.to_owned(), font.clone(), egui::Color32::WHITE)
                        .size()
                        .x
                }
            };
            let widths: Vec<f32> = v
                .buttons
                .iter()
                .map(|b| (text_w(&b.text) + v.size.metrics().pad_x * 1.5).max(v.size.metrics().button_h))
                .collect();
            cobolt_forms::snackbar::notification_size(
                v.size,
                v.icon.as_ref().map(|_| v.icon_size),
                &v.text,
                &widths,
                &text_w,
                v.text_wrap,
                surf,
            )
        };
        self.snackbars.layout(surf, &measure, now);

        // Paint newest LAST so it sits over its neighbours, and collect each
        // notification's clickable rects for hit-testing — its built-in close
        // plus the developer's own buttons. The stack decides what is drawn and
        // how far through its entrance, movement or fade it is; the host only
        // turns that into a rect and an alpha.
        let mut hits: Vec<(u64, cobolt_forms::paint::SnackbarPaint)> = Vec::new();
        for d in self.snackbars.to_draw(now) {
            let rect = egui::Rect::from_min_size(
                egui::Pos2::new(d.rect.x as f32, d.rect.y as f32),
                egui::Vec2::new(d.rect.w as f32, d.rect.h as f32),
            );
            // The entrance zoom is about the notification's own centre, so it
            // grows in place rather than sliding out of its slot.
            let rect = egui::Rect::from_center_size(rect.center(), rect.size() * d.scale);
            let out = cobolt_forms::paint::draw_snackbar(
                &painter,
                rect,
                d.visual,
                None,
                d.alpha,
                snack_pointer,
            );
            // A remnant is already closed: its close and buttons are not
            // clickable.
            if d.interactive {
                hits.push((d.id, out));
            }
        }

        // A click on the close or a button. The notification is not a control,
        // so this is not an `interact` on a widget id — it is a hit test
        // against the rects the painter just reported, which is the same thing
        // the toolbar does. The close is always on (055 follow-up, operator
        // 2026-09-03) and dismisses only ITS notification — unlike
        // `DismissAll()`, which is scoped to the whole control.
        if ui.input(|i| i.pointer.primary_clicked()) {
            if let Some(pos) = ui.ctx().pointer_interact_pos() {
                'outer: for (id, paint) in hits.iter().rev() {
                    match paint.hit_test(pos) {
                        Some(cobolt_forms::paint::SnackHit::Close) => {
                            self.snackbars.dismiss(*id, cobolt_forms::snackbar::DismissReason::User);
                            break 'outer;
                        }
                        Some(cobolt_forms::paint::SnackHit::Button(idx)) => {
                            self.snackbars.click_button(*id, idx);
                            break 'outer;
                        }
                        None => {}
                    }
                }
            }
        }

        // Report. A notification is raised BY a control, so its events are that
        // control's — a handler binds them in the designer like any other.
        for ev in self.snackbars.drain_events() {
            use crate::snackbar_stack::SnackEvent as E;
            let (ctrl_id, name, value) = match ev {
                E::Shown { ctrl_id, .. } => (ctrl_id, "onShown", String::new()),
                E::Timeout { ctrl_id, .. } => (ctrl_id, "onTimeout", String::new()),
                E::Closing { ctrl_id, reason, .. } => {
                    (ctrl_id, "onClosing", reason.as_str().to_owned())
                }
                E::Closed { ctrl_id, reason, .. } => {
                    (ctrl_id, "onClosed", reason.as_str().to_owned())
                }
                // WHICH button was pressed arrives as `LastButtonId` /
                // `LastButtonIndex` on the Snackbar, written BEFORE the event so
                // a handler reading `SNACK-1::LastButtonId` already sees this
                // press — exactly the rule a ToolBar's `LastButton` follows. The
                // event also carries id and index TAB-separated as its value,
                // the encoding a TreeView node event uses.
                E::ButtonClick { ctrl_id, button_id, index, .. } => {
                    // The button's place counts from 1, as COBOL does (the
                    // stack's own index counts from 0).
                    let place = index + 1;
                    for (k, v) in [
                        ("LastButtonId", button_id.clone()),
                        ("LastButtonIndex", place.to_string()),
                    ] {
                        self.state_entry_mut(&ctrl_id).set(k, v.clone());
                        let _ = self.input_tx.send(cobolt_runtime::channels::StateUpdate::new(
                            ctrl_id.clone(),
                            k.to_string(),
                            v,
                        ));
                    }
                    (ctrl_id, "onButtonClick", format!("{button_id}\t{place}"))
                }
            };
            let mut fe = FormEvent::new(ctrl_id, name);
            fe.value = value;
            self.send_event(fe);
        }

        // A live notification is a reason to keep painting: its timeout has to
        // elapse even when nothing else on the form is moving. An effect in
        // flight — or one queued behind it — is a reason to paint at screen
        // rate: 50 ms would draw a 300 ms glide six times, which steps rather
        // than moves, and would let each queued arrival start up to a frame late.
        if !self.snackbars.is_empty() {
            let interval = if self.snackbars.is_animating(now) { 16 } else { 50 };
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(interval));
        }
    }

    pub(crate) fn send_event(&mut self, ev: FormEvent) {
        // An unbound motion event is pure backlog: nothing runs when it is
        // dispatched, and the interpreter pays a full `COBOL-WAIT-EVENT` round
        // trip to find that out. See `motion_bound`.
        if cobolt_forms::diagnostics::is_motion_event(&ev.event_id)
            && !self.motion_bound.contains(&motion_key(&ev.ctrl_id, &ev.event_id))
        {
            return;
        }
        crate::diagnostics::trace_event("send", &ev.ctrl_id, &ev.event_id, ev.instance_index);
        if self.ev_tx.send(ev).is_ok() {
            self.pending.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// See [`crate::state::state_entry_mut`].
    pub(crate) fn state_entry_mut(&mut self, key: &str) -> &mut CtrlState {
        state_entry_mut(&mut self.state, &self.controls, key)
    }

    /// How far the interpreter may fall behind before a due Timer tick is dropped.
    ///
    /// Coalescing exists so a handler slower than its interval cannot be handed an
    /// ever-growing queue of ticks (WinForms semantics: a Timer never repays missed
    /// time). It used to drop a tick whenever ANY event was outstanding, which
    /// became a starvation bug the moment observer events arrived (1.61.75): a
    /// Timer handler that writes a Gauge or a Label queues one or two `onChange`
    /// events per tick, so there was almost always something outstanding and the
    /// next tick was dropped — a Timer that quietly stops after a while, which is
    /// exactly what was reported.
    ///
    /// A handler that is keeping up never has this many events outstanding; one
    /// that does is genuinely behind and should not be given more ticks. If this
    /// ever needs to be sharper, the right rule is to count outstanding TICKS
    /// rather than events — that needs the interpreter to report what kind of event
    /// it consumed, which this does not.
    pub(crate) const TICK_COALESCE_BACKLOG: usize = 8;

    /// Forward one frame's property updates and UI events to the interpreter,
    /// coalescing Timer ticks against the backlog. Returns whether anything was
    /// sent.
    ///
    /// Shared by the root and child paths for the reason
    /// [`Self::apply_interpreter_update`] gives: two consumers of one
    /// `RenderOutput` drift, and two of them already had.
    /// Turn off every other radio in `id`'s group, and tell the program.
    ///
    /// A radio group holds at most one selection, and that has to be true
    /// whoever made it: the renderer clears siblings on a CLICK, and this
    /// clears them on a write from code. Without it a `SET Rad-PIX::Selected TO
    /// TRUE` lit PIX and left Boleto lit beside it (operator, 2026-09-17).
    ///
    /// Both directions are updated. The host's own state is what the next frame
    /// paints from; the `StateUpdate` back to the interpreter is what makes
    /// `Rad-Boleto::IsSelected()` answer 0 afterwards, which it would not if
    /// only the picture were corrected.
    ///
    /// Every spelling the sibling might be read under is cleared, because a
    /// handler may ask with either and both must agree — the same rule the
    /// renderer's click path follows.
    fn clear_radio_siblings(&mut self, id: &str) {
        let Some(group) = self
            .controls
            .iter()
            .find(|c| c.id == id)
            .map(cobolt_forms::model::radio_group_key)
        else {
            return;
        };
        let siblings: Vec<String> = self
            .controls
            .iter()
            .filter(|c| matches!(c.control_type, cobolt_forms::ControlType::RadioButton))
            .filter(|c| c.id != id)
            .filter(|c| cobolt_forms::model::radio_group_key(c) == group)
            .map(|c| c.id.clone())
            .collect();
        for sibling in siblings {
            for prop in [
                cobolt_forms::model::SELECTED_PROP,
                cobolt_forms::model::CHECKED_PROP,
                "Value",
            ] {
                self.state_entry_mut(&sibling).set(prop, "0".to_owned());
                let _ = self.input_tx.send(StateUpdate::new(
                    sibling.clone(),
                    prop.to_owned(),
                    "0".to_owned(),
                ));
            }
        }
    }

    pub(crate) fn forward_interaction(
        &mut self,
        prop_updates: &[(String, String, String)],
        events: Vec<cobolt_forms::render::UiEvent>,
        // 051 R19/R28 — this form is waiting on a modal child (or the
        // debugger). `ui.disable()` already refuses input to egui WIDGETS,
        // but the engine detects a control's click from RAW pointer state
        // and rect containment (`render.rs`, "One press, one click"), which
        // `disable()` does not touch — so a click on a blocked form still
        // produced an `onClick`, which queued on the interpreter's event
        // channel while it sat inside `OpenFormSync`, and replayed the moment
        // the modal closed: every click on the caller's "open" button while
        // the modal was up opened it once more (operator, 2026-09-19). A
        // blocked form takes NO operator input; only the non-interaction
        // events keep flowing.
        blocked: bool,
    ) -> bool {
        let mut sent = false;
        let (prop_updates, events): (&[(String, String, String)], Vec<_>) = if blocked {
            (
                &[],
                events
                    .into_iter()
                    .filter(|ev| Self::event_flows_while_blocked(&ev.event))
                    .collect(),
            )
        } else {
            (prop_updates, events)
        };
        // Live values first, so a handler woken by the event that follows reads the
        // value that caused it.
        for (id, key, val) in prop_updates {
            self.state_entry_mut(id).set(key, val.clone());
            let _ = self
                .input_tx
                .send(StateUpdate::new(id.clone(), key.clone(), val.clone()));
            sent = true;
        }
        let backlog = self.pending.load(Ordering::Relaxed) >= Self::TICK_COALESCE_BACKLOG;
        for ev in events {
            // User events (clicks, edits, focus, quit) are never dropped.
            if ev.event.eq_ignore_ascii_case("onTick") && backlog {
                continue;
            }
            // Instanced repeating-group members are drawn with the id
            // "group.group-N.member" — dispatch to the designed (base) member id,
            // forwarding the 1-based instance index so the handler receives
            // CONTROL-ARRAY-INDEX.
            let (dispatch_id, inst) = if ev.ctrl_id.contains('.') {
                let base = ev
                    .ctrl_id
                    .rsplit('.')
                    .next()
                    .unwrap_or(&ev.ctrl_id)
                    .to_string();
                let inst = {
                    let parts: Vec<&str> = ev.ctrl_id.split('.').collect();
                    if parts.len() >= 2 {
                        parts[1]
                            .rsplit('-')
                            .next()
                            .and_then(|s| s.parse::<usize>().ok())
                            .unwrap_or(0)
                    } else {
                        0
                    }
                };
                (base, inst)
            } else {
                (ev.ctrl_id.clone(), 0)
            };
            // `CloneEvents` off on the repeating group: only the designed
            // card (instance 1) runs the handlers — its clones are display
            // only. The flag was read by nothing, so every clone always fired
            // (property audit, 2026-09-25).
            if inst > 1 {
                let group_id = ev.ctrl_id.split('.').next().unwrap_or("");
                let clones_fire = self
                    .controls
                    .iter()
                    .find(|c| c.id.eq_ignore_ascii_case(group_id))
                    .and_then(|g| g.get_prop("CloneEvents"))
                    .map(|v| v.as_bool())
                    .unwrap_or(true);
                if !clones_fire {
                    continue;
                }
            }
            // The event's VALUE travels with it. It was dropped here, so a
            // TreeView handler for onNodeCheck/onNodeCollapse/onNodeExpand
            // could not tell which node had moved — those events write no
            // SelectedNode, and nothing else carried the answer (operator,
            // 2026-08-22).
            let mut fe = FormEvent::new(dispatch_id, ev.event).with_index(inst);
            if let Some(v) = ev.value {
                fe = fe.with_value(v);
            }
            self.send_event(fe);
            sent = true;
        }
        sent
    }

    /// The events a BLOCKED form still forwards: the ones no operator action
    /// produced. `onLoad` fires once per control and would otherwise be lost
    /// for good; `onTick` is a Timer's clock, and a handler parked in
    /// `OpenFormSync` sees the backlog coalesced anyway. Everything else is a
    /// mouse or keyboard acting on a form that is not taking input.
    fn event_flows_while_blocked(event: &str) -> bool {
        event.eq_ignore_ascii_case("onLoad") || event.eq_ignore_ascii_case("onTick")
    }

    /// Everything one frame's interaction asks of the PLATFORM rather than of the
    /// form's COBOL: a FileDropZone's native file picker, and a toolbar button's
    /// platform action (print, share, capture, the clipboard, another process).
    /// Returns whether anything happened, for frame scheduling.
    ///
    /// **One place on purpose**, the same reason as
    /// [`Self::apply_interpreter_update`]: this ran on the ROOT form's path only,
    /// so in a child window or a ContentPane occupant every platform toolbar
    /// action and every click-to-browse was silently dead. Two consumers of one
    /// `RenderOutput` will always drift; there is now one.
    /// The two lists are taken separately rather than as a whole `RenderOutput`
    /// because both callers have already moved its `events` out by this point.
    pub(crate) fn run_platform_requests(
        &mut self,
        ctx: &egui::Context,
        file_pickers: &[String],
        csv_exports: &[String],
        toolbar_actions: &[(String, String, String)],
        pre_focus: Option<egui::Id>,
        // The id space the surface those requests came from was rendered in
        // (`None` = the form's own). The clipboard verbs match egui's focused
        // widget back to a control, and a control drawn in the SideMenu footer
        // carries the footer's id — matching it against the plain one would
        // report "no text field has focus" for a field the operator is in.
        scope: Option<egui::Id>,
    ) -> bool {

        let mut acted = false;

        // FileDropZone click → native picker (spec 039 T4). `cobolt-forms` has no
        // native-dialog dependency by design (see render.rs's
        // `RenderOutput::file_picker_requests` doc comment) — this crate owns the
        // non-blocking dialog (spec 042 R25).
        for id in file_pickers {
            let key = format!("filedropzone:{id}");
            crate::file_dialog::begin(ctx, &key, crate::file_dialog::DialogSpec::open());
        }

        // The DataGrid CSV button asking where to write. Same division of labour
        // as the FileDropZone above: the engine knows the button was pressed and
        // owns no dialog, this crate owns the dialog and no CSV, and the runtime
        // owns the one definition of what a CSV of a grid is.
        //
        // Before this the button exported straight to `<control-id>.csv` in the
        // working directory — a destination the operator never chose and a
        // packaged application cannot predict (operator, 2026-09-16).
        for id in csv_exports {
            let key = format!("datagridcsv:{id}");
            // `CSVExportPath` still leads when the developer set one: it becomes
            // the suggested name and folder rather than being overridden.
            let configured = self
                .controls
                .iter()
                .find(|c| &c.id == id)
                .and_then(|c| c.get_prop("CSVExportPath"))
                .map(|v| v.as_str().trim().to_owned())
                .filter(|s| !s.is_empty());
            let suggested = configured.unwrap_or_else(|| format!("{id}.csv"));
            let suggested = Path::new(&suggested);
            let mut spec = crate::file_dialog::DialogSpec::save().filter("CSV", &["csv"]);
            if let Some(name) = suggested.file_name().and_then(|n| n.to_str()) {
                spec = spec.file_name(name);
            }
            if let Some(dir) = suggested.parent().filter(|p| !p.as_os_str().is_empty()) {
                spec = spec.directory(dir);
            }
            crate::file_dialog::begin(ctx, &key, spec);
            acted = true;
        }
        let grid_ids: Vec<String> = self
            .controls
            .iter()
            .filter(|c| matches!(c.control_type, cobolt_forms::ControlType::DataGrid))
            .map(|c| c.id.clone())
            .collect();
        for id in grid_ids {
            let key = format!("datagridcsv:{id}");
            let Some(answer) = crate::file_dialog::take(&key) else {
                continue;
            };
            acted = true;
            // Cancelled: nothing is written and no export is raised. A save
            // panel the operator dismissed must not still produce a file.
            let Some(path) = answer else { continue };
            // Destination FIRST, then the request — the interpreter reads
            // `CSVExportPath` when it carries the export out, so the order is
            // what makes the chosen path the one actually used.
            for (prop, value) in [
                ("CSVExportPath", path.display().to_string()),
                ("_ExportCSVRequested", "1".to_owned()),
            ] {
                self.state_entry_mut(&id).set(prop, value.clone());
                let _ = self
                    .input_tx
                    .send(StateUpdate::new(id.clone(), prop.to_owned(), value));
            }
            self.wake_for_input(&id);
        }
        let file_drop_zone_ids: Vec<String> = self
            .controls
            .iter()
            .filter(|c| matches!(c.control_type, cobolt_forms::ControlType::FileDropZone))
            .map(|c| c.id.clone())
            .collect();
        for id in file_drop_zone_ids {
            let key = format!("filedropzone:{id}");
            if let Some(Some(path)) = crate::file_dialog::take(&key) {
                // Browsing goes through the SAME intake as a drop — the zone's
                // extensions, size limit and destination folder — so a file is
                // judged by one set of rules however it arrived.
                // The zone AS IT IS NOW: a value the COBOL set at run time wins
                // over the designed one. Reading only the design dropped every
                // picked file on the floor when the form set DestinationFolder
                // itself — the file was "accepted" with nowhere to go
                // (operator, 2026-09-27 — PowerChat's Documents upload). The
                // drag-drop path in the renderer already reads the live control.
                let ctrl = self.controls.iter().find(|c| c.id == id);
                let live = self.state.get(&id);
                let prop = |key: &str| -> String {
                    live.and_then(|s| {
                        s.props
                            .iter()
                            .find(|(k, _)| k.eq_ignore_ascii_case(key))
                            .map(|(_, v)| v.clone())
                    })
                    .or_else(|| {
                        ctrl.and_then(|c| c.get_prop(key))
                            .map(|v| v.as_str().to_owned())
                    })
                    .unwrap_or_default()
                };
                let bool_prop = |key: &str, default: bool| -> bool {
                    match live.and_then(|s| {
                        s.props
                            .iter()
                            .find(|(k, _)| k.eq_ignore_ascii_case(key))
                            .map(|(_, v)| v.clone())
                    }) {
                        Some(v) => cobolt_forms::PropValue::String(v).as_bool(),
                        None => ctrl
                            .and_then(|c| c.get_prop(key))
                            .map(|v| v.as_bool())
                            .unwrap_or(default),
                    }
                };
                // `apply_drop` also decides whether this copies now or only stages
                // for the form to confirm — the same answer the drag-drop path gets.
                let writes = cobolt_forms::dropzone::apply_drop(
                    &id,
                    &[path.display().to_string()],
                    cobolt_forms::dropzone::ZoneRules {
                        filter: &prop("AllowedExtensions"),
                        max_kb: prop("MaximumFileSizeKB").parse::<i64>().unwrap_or(0),
                        destination: &prop("DestinationFolder"),
                        stage_only: bool_prop("StageOnly", false),
                        list_id: &prop("FileListControl"),
                        already_staged: &self
                            .state
                            .get(&id)
                            .and_then(|s| {
                                s.props
                                    .iter()
                                    .find(|(k, _)| k.eq_ignore_ascii_case("StagedFiles"))
                                    .map(|(_, v)| v.clone())
                            })
                            .unwrap_or_default(),
                    },
                );
                for (target, key, value) in &writes.updates {
                    self.state_entry_mut(target).set(key, value.clone());
                    let _ = self.input_tx.send(StateUpdate::new(
                        target.clone(),
                        key.clone(),
                        value.clone(),
                    ));
                }
                if writes.accepted > 0 {
                    self.send_event(FormEvent::new(id.clone(), "onFilesDropped".to_owned()));
                }
                if writes.rejected > 0 {
                    self.send_event(FormEvent::new(id, "onFilesRejected".to_owned()));
                }
                acted = true;
            }
        }

        // ── Toolbar buttons whose action is the platform's work ───────────────
        //
        // The renderer already fired the button's `onClick`, so the form has heard
        // about the press either way; this is the deed itself.
        for (ctrl_id, button_id, action) in toolbar_actions {
            let parsed = cobolt_forms::toolbar::ToolbarAction::parse(action);
            // Copy/Cut/Paste act on whichever control has keyboard focus. egui
            // reports that as a widget id, and a control's TextEdit is built with
            // `Id::new(("rt_ctrl", <control id>))` — so the focused control is
            // found by matching that back.
            //
            // The very click that pressed the button SURRENDERS that focus:
            // egui 0.36 defaults to `SurrenderFocusOn::Clicks`, so by the time
            // this runs, live focus is gone and every clipboard verb reported
            // "No text field has focus" (operator, 2026-08-23 — "copy, paste
            // are doing nothing"). `pre_focus` is the focus as it stood BEFORE
            // this frame's widgets processed the click — the field the user
            // means — and is the fallback when the live answer is empty.
            let focused = ctx
                .memory(|m| m.focused())
                .or(pre_focus)
                .and_then(|focus| {
                    self.controls.iter().find_map(|c| {
                        let widget = cobolt_forms::render::control_widget_id(scope, c.id.as_str());
                        (widget == focus).then(|| {
                            // The live text when the field was edited; the
                            // DESIGNED text otherwise — an untouched field's
                            // Copy used to copy "".
                            let text = self
                                .state
                                .get(&c.id)
                                .and_then(|s| {
                                    s.props.iter().find_map(|(k, v)| {
                                        (k.eq_ignore_ascii_case("Text")
                                            || k.eq_ignore_ascii_case("Value"))
                                        .then(|| v.clone())
                                    })
                                })
                                .or_else(|| {
                                    c.get_prop("Text")
                                        .or_else(|| c.get_prop("Value"))
                                        .map(|v| v.as_str().to_owned())
                                })
                                .unwrap_or_default();
                            (c.id.clone(), text)
                        })
                    })
                });
            let focused_ref = focused
                .as_ref()
                .map(|(id, text)| cobolt_forms::toolbar_actions::Focused {
                    control_id: id.as_str(),
                    text: text.clone(),
                    widget_id: cobolt_forms::render::control_widget_id(scope, id.as_str()),
                });

            let (outcome, new_text) = self.toolbar_runner.perform(ctx, &parsed, focused_ref);
            self.note_action_outcome(ctx, &outcome);
            // A Cut or a Paste changed the focused field: write it back the way a
            // keystroke would have, so the form sees it.
            if let (Some(text), Some((target, _))) = (new_text, focused) {
                self.state_entry_mut(&target).set("Text", text.clone());
                let _ = self.input_tx.send(StateUpdate::new(
                    target.clone(),
                    "Text".to_owned(),
                    text,
                ));
                self.send_event(FormEvent::new(target, "onChange".to_owned()));
            }
            let _ = (&ctrl_id, &button_id);
            acted = true;
        }
        // A window capture asked for on an earlier frame finishes here.
        if let Some(outcome) = self.toolbar_runner.poll_capture(ctx) {
            self.note_action_outcome(ctx, &outcome);
            acted = true;
        }
        acted
    }

    /// Keep a platform-action outcome to show in the window for a few
    /// seconds. `Pending` is skipped — its completion reports itself.
    fn note_action_outcome(
        &mut self,
        ctx: &egui::Context,
        outcome: &cobolt_forms::toolbar_actions::Outcome,
    ) {
        use cobolt_forms::toolbar_actions::Outcome;
        if matches!(outcome, Outcome::Pending(_)) {
            return;
        }
        let now = ctx.input(|i| i.time);
        self.action_notice = Some((outcome.message().to_owned(), outcome.is_error(), now));
    }

    /// Paint the latest platform-action outcome as a small bottom-anchored
    /// notice for a few seconds, so a Failed print or an empty-clipboard
    /// paste is VISIBLE instead of a line on stderr. Colours are fixed, not
    /// taken from the theme — a glass theme's ambient values are exactly what
    /// made text unreadable before.
    pub(crate) fn show_action_notice(&mut self, ctx: &egui::Context) {
        const NOTICE_SECONDS: f64 = 4.0;
        let Some((message, is_error, shown_at)) = &self.action_notice else {
            return;
        };
        let now = ctx.input(|i| i.time);
        if now - shown_at > NOTICE_SECONDS {
            self.action_notice = None;
            return;
        }
        let (fg, bg) = if *is_error {
            (
                egui::Color32::WHITE,
                egui::Color32::from_rgba_unmultiplied(150, 40, 40, 230),
            )
        } else {
            (
                egui::Color32::WHITE,
                egui::Color32::from_rgba_unmultiplied(30, 60, 90, 230),
            )
        };
        egui::Area::new(egui::Id::new(("toolbar-action-notice", self.form_name.as_str())))
            .anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(0.0, -18.0))
            .order(egui::Order::Foreground)
            .interactable(false)
            .show(ctx, |ui| {
                egui::Frame::NONE
                    .fill(bg)
                    .corner_radius(6.0)
                    .inner_margin(egui::Margin::symmetric(10, 6))
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new(message.as_str()).color(fg).size(13.0));
                    });
            });
        ctx.request_repaint(); // keep the clock running so the notice expires
    }

    /// Apply one interpreter → UI property update, and fire the OBSERVER events
    /// the change earns.
    ///
    /// **One place on purpose.** There are two frame paths — the ROOT form's
    /// (`FormHost::ui_impl`, what `rcrun run-form` shows) and a CHILD window's or
    /// ContentPane occupant's ([`Self::child_frame`]) — and they had drifted apart
    /// in opposite directions:
    ///
    /// * the observer events (1.61.71) were added to the child path only, so a
    ///   Timer doing `MOVE 5 TO KNOB-1::Value` in the MAIN form still fired
    ///   nothing — the very bug that change set out to fix;
    /// * the toolbar-button write routing (1.61.74) was added to the root path
    ///   only, so recolouring a button in a child form did nothing.
    ///
    /// Both call this now, so neither can be fixed without the other.
    /// A write to the FORM's own window properties — `me::Title`,
    /// `BackgroundColor`, `Transparency`, `Width`, `Height`, `X`, `Y` — made
    /// visible. They were readable and writable from COBOL and changed only the
    /// property store: no command ever reached the window, and the backdrop
    /// kept the colour and transparency captured at open (property audit,
    /// 2026-09-26). The backdrop values apply on every surface; the window
    /// commands only to the window this body owns (`window`, its viewport) —
    /// `None` in the shell's pane, where the shell owns it. Returns whether it
    /// was such a write (the update is still stored as usual either way).
    pub(crate) fn apply_form_window_update(
        &mut self,
        ctx: &egui::Context,
        u: &StateUpdate,
        window: Option<egui::ViewportId>,
    ) -> bool {
        if !u.ctrl_id.trim().eq_ignore_ascii_case(&self.form_object) {
            return false;
        }
        if self.write_form_layout(&u.prop, &u.value) {
            return true;
        }
        let num = || u.value.trim().parse::<f32>().ok();
        match u.prop.to_ascii_lowercase().as_str() {
            "backgroundcolor" => self.bg_hex = u.value.trim().to_owned(),
            "transparency" => {
                if let Some(t) = num() {
                    self.transparency = t.clamp(0.0, 100.0) as u8;
                }
            }
            // Rounds the window while it has no title bar. Only a window that
            // was created see-through can show it — one whose form was
            // DESIGNED with a radius (see `see_through_window`).
            "cornerradius" => {
                if let Some(r) = num() {
                    self.corner_radius = r.clamp(0.0, 255.0) as u32;
                }
            }
            "title" => {
                if let Some(vp) = window {
                    ctx.send_viewport_cmd_to(vp, egui::ViewportCommand::Title(u.value.trim().to_owned()));
                }
            }
            "width" | "height" => {
                let Some(v) = num().filter(|v| *v >= 64.0) else { return true };
                let v = v.min(cobolt_forms::model::FORM_MAX_SIZE as f32);
                if u.prop.eq_ignore_ascii_case("width") {
                    self.form_size.x = v;
                } else {
                    self.form_size.y = v;
                }
                if let Some(vp) = window {
                    ctx.send_viewport_cmd_to(vp, egui::ViewportCommand::InnerSize(self.form_size));
                }
            }
            "x" | "y" => {
                if let (Some(v), Some(vp)) = (num(), window) {
                    let at = ctx
                        .input_for(vp, |i| i.viewport().outer_rect.map(|r| r.min))
                        .unwrap_or(egui::Pos2::ZERO);
                    let to = if u.prop.eq_ignore_ascii_case("x") { egui::pos2(v, at.y) } else { egui::pos2(at.x, v) };
                    ctx.send_viewport_cmd_to(vp, egui::ViewportCommand::OuterPosition(to));
                }
            }
            _ => return false,
        }
        true
    }

    pub(crate) fn apply_interpreter_update(&mut self, u: StateUpdate, diagnostics: bool) {
        let key = if u.instance_index > 0 {
            match self.array_member_group(&u.ctrl_id) {
                Some((member_id, group_id)) => cobolt_forms::render::member_instance_id(
                    &group_id,
                    &member_id,
                    u.instance_index,
                ),
                None => self.resolve_ctrl_key(&u.ctrl_id),
            }
        } else {
            self.resolve_ctrl_key(&u.ctrl_id)
        };
        // R27 — the live trace: which designed control this write landed on, or
        // NO SUCH CONTROL with the ids that do exist. The routing itself is
        // unchanged; the trace only reports it.
        if diagnostics {
            let matched = self
                .state
                .keys()
                .find(|k| k.eq_ignore_ascii_case(&key))
                .cloned();
            let known: Vec<&str> = if matched.is_none() {
                self.state.keys().map(|s| s.as_str()).collect()
            } else {
                Vec::new()
            };
            crate::diagnostics::trace_state_update(
                &u.ctrl_id,
                &u.prop,
                &u.value,
                matched.as_deref(),
                &known,
            );
        }
        // A write to a toolbar BUTTON is not a write to a control: a button's
        // appearance comes from its toolbar's stored definition and from nowhere
        // else, so the change belongs in that definition. Rewriting it there means
        // the renderer needs to know nothing about live button state — it reads
        // `ToolbarLayout` off the live control as it always has, and the next
        // frame is already correct.
        //
        // The interpreter has already refused anything but a colour, a tooltip or
        // Enabled, out loud; this only carries an allowed write home.
        if self.apply_toolbar_button_write(&key, &u.prop, &u.value) {
            return;
        }
        // 055 — `Show()` and `DismissAll()` reach the host as pseudo-property
        // writes, the way `PlayAnimation` already does. They are COMMANDS, not
        // state: storing them would leave `_ShowSnackbar` sitting in the control's
        // property map, where the next `mint` would read it back as if the
        // developer had set it.
        if self.snackbar_command(&key, &u.prop, &u.value) {
            return;
        }
        // 058 R18.1 — the Viewer asking where to save. Same shape and the same
        // reason as the Snackbar's `Show()` above: a COMMAND, not state.
        if self.viewer_save_as_request(&key, &u.prop, &u.value) {
            return;
        }
        // 058 R19/R20 — and the two that hand the document to the platform.
        if self.viewer_os_request(&key, &u.prop, &u.value) {
            return;
        }
        // An OBSERVER event reports that a value is now different, whoever made it
        // different — so a Timer handler doing `MOVE 5 TO KNOB-1::Value` has to
        // fire the Knob's `onValueChanged` exactly as a drag does.
        //
        // Only when the value actually CHANGED: an observer that fires on a write
        // of the same value is a spurious event, and — since a handler may well
        // write the property it was woken for — it is also what stops the obvious
        // feedback loop.
        //
        // Passive events (`onClick`, `onMouseDown`, `onGotFocus`) are never raised
        // here. There is no user act to report.
        let observers: Vec<&'static str> = self
            .controls
            .iter()
            .find(|c| c.id == key)
            .map(|c| c.control_type.observer_events_for(&u.prop))
            .unwrap_or_default();
        let changed = if observers.is_empty() {
            false
        } else {
            self.state
                .get(&key)
                .and_then(|s| {
                    s.props
                        .iter()
                        .find(|(k, _)| k.eq_ignore_ascii_case(&u.prop))
                        .map(|(_, v)| v != &u.value)
                })
                // Nothing stored yet: the interpreter's seeding pass writes every
                // designed value on startup, and a form must not wake to a burst
                // of change events for values that never changed. Compare against
                // the DESIGN value instead.
                .unwrap_or_else(|| {
                    self.controls
                        .iter()
                        .find(|c| c.id == key)
                        .and_then(|c| c.get_prop(&u.prop).map(|v| v.as_str() != u.value))
                        .unwrap_or(false)
                })
        };
        // One radio at a time, whoever turned it on.
        //
        // The renderer clears a group's other buttons when one is CLICKED, but
        // a `SET Rad-PIX::Selected TO TRUE` never passes through it — the write
        // arrives here, straight from the interpreter — so the button that was
        // already lit stayed lit and the group showed two selected at once
        // (operator, 2026-09-17). Exclusivity has to hold for every writer, and
        // this is the one place every code-driven write passes through.
        let turned_on = !matches!(u.value.trim(), "" | "0" | "false" | "FALSE");
        let is_radio_state = self
            .controls
            .iter()
            .find(|c| c.id == key)
            .is_some_and(|c| {
                matches!(c.control_type, cobolt_forms::ControlType::RadioButton)
                    && cobolt_forms::model::is_toggle_state_property(&c.control_type, &u.prop)
            });
        // 056 R38/R64 — on a responsive form a write to a layout property lands
        // on the design the layout reads, and wins over any breakpoint.
        if self.responsive.is_some() {
            self.write_design(&key, &u.prop, &u.value);
        }
        // A toggle's state is written under BOTH of its spellings, so the two
        // can never disagree. A click already writes both (the renderer's
        // `push_toggle_state`), and so does `clear_radio_siblings`; a write
        // from code arrives as one spelling only. Stored alone, it sat beside a
        // stale other spelling a click had left — a radio holding `Selected` 1
        // and `Checked` 0 — and the paint folds both onto one key in HashMap
        // order, so the stale 0 usually won: `Rad-Pix::Select()` after the
        // operator had clicked another payment ran the handler (the label said
        // PIX) and left every circle empty (operator, 2026-10-06).
        let toggle_state = self
            .controls
            .iter()
            .find(|c| c.id == key)
            .is_some_and(|c| cobolt_forms::model::is_toggle_state_property(&c.control_type, &u.prop));
        if toggle_state {
            let entry = self.state_entry_mut(&key);
            entry.set(cobolt_forms::model::SELECTED_PROP, u.value.clone());
            entry.set(cobolt_forms::model::CHECKED_PROP, u.value);
        } else {
            self.state_entry_mut(&key).set(&u.prop, u.value);
        }
        if is_radio_state && turned_on {
            self.clear_radio_siblings(&key);
        }
        if changed {
            for event in observers {
                self.send_event(FormEvent::new(key.clone(), event.to_owned()));
            }
        }
    }

    /// Spec 056 R84 — a program's write to one of the form's responsive
    /// properties, taking effect on this frame's layout pass. `Responsive`
    /// switches the layout on or off; `Breakpoint` pins it to a breakpoint
    /// (`SPACES` unpins); `FontScale` pins the total font factor (0 unpins);
    /// `Breakpoints` replaces the table (overrides follow their names); any
    /// layout-bag property is written into the form's layout. Returns whether
    /// `prop` was one of them.
    fn write_form_layout(&mut self, prop: &str, value: &str) -> bool {
        let key = prop.trim();
        let lower = key.to_ascii_lowercase();
        if lower == "responsive" {
            let on = !matches!(value.trim(), "" | "0" | "false" | "FALSE");
            if let Some(spec) = self.responsive.as_mut().or(self.responsive_off.as_mut()) {
                spec.switched_on = on;
            }
            self.settle_layout_switch();
            return true;
        }
        let bag_key = cobolt_forms::layout::defaults::form_defaults()
            .into_iter()
            .map(|(k, _)| k)
            .find(|k| k.eq_ignore_ascii_case(key));
        if !matches!(lower.as_str(), "breakpoint" | "fontscale" | "breakpoints") && bag_key.is_none() {
            return false;
        }
        // Written while switched off, it waits in the stored design.
        let Some(spec) = self.responsive.as_mut().or(self.responsive_off.as_mut()) else { return true };
        match lower.as_str() {
            "breakpoint" => {
                let v = value.trim();
                spec.pinned_breakpoint = (!v.is_empty()).then(|| v.to_owned());
            }
            "fontscale" => {
                spec.pinned_font_scale = value.trim().parse::<f32>().ok().filter(|v| *v > 0.0);
            }
            "breakpoints" => {
                spec.breakpoints = cobolt_forms::layout::breakpoints::from_text(value, &spec.breakpoints);
            }
            _ => {
                if let Some(k) = bag_key {
                    // Spec 081 R14 — only 0–7 is a scaling style. Anything
                    // else is refused: the form keeps its style, and the
                    // program reads that style back, not what it wrote.
                    if k == "ObsoleteScalingStyle" && !valid_scaling_style(value) {
                        let kept = cobolt_forms::layout::scale::style(&cobolt_forms::layout::props::FormBag(&spec.layout));
                        let _ = self.input_tx.send(StateUpdate::new(self.form_object.clone(), k.to_owned(), kept.to_string()));
                        return true;
                    }
                    spec.layout.insert(k.to_owned(), cobolt_forms::PropValue::String(value.to_owned()));
                }
            }
        }
        self.settle_layout_switch();
        true
    }

    /// Spec 081 R4 — lay the form out exactly while its design says so:
    /// `Responsive` on, or an obsolete scaling style set.
    fn settle_layout_switch(&mut self) {
        let want = self.responsive.as_ref().or(self.responsive_off.as_ref()).is_some_and(|s| s.lays_out());
        if want && self.responsive.is_none() {
            self.responsive = self.responsive_off.take();
        } else if !want && self.responsive.is_some() {
            self.responsive_off = self.responsive.take();
        }
    }

    /// Spec 056 R37, R46, R47 — tell the program what the last layout did:
    /// every control whose laid-out `X`/`Y`/`Width`/`Height` differs from what
    /// it was last told (nothing at the designed size), then the form's
    /// `Breakpoint` and `FontScale` when they change, and `onBreakpointChanged`
    /// when the breakpoint does — never for the one the form opens in. It runs
    /// after the frame's layout, so a settling resize finds all of it already
    /// sent when its `onResize` is raised on the next frame.
    pub(crate) fn mirror_layout(&mut self) {
        let Some(layout) = self.last_layout.as_ref() else { return };
        let told = self
            .mirrored
            .get_or_insert_with(|| self.controls.iter().map(|c| (c.id.clone(), c.rect)).collect());
        for (id, r) in &layout.rects {
            let now = cobolt_forms::model::Rect::new(
                r.x.round() as i32,
                r.y.round() as i32,
                r.w.round() as i32,
                r.h.round() as i32,
            );
            let was = told.insert(id.clone(), now);
            for (prop, v, old) in [
                ("X", now.x, was.map(|w| w.x)),
                ("Y", now.y, was.map(|w| w.y)),
                ("Width", now.w, was.map(|w| w.w)),
                ("Height", now.h, was.map(|w| w.h)),
            ] {
                if old != Some(v) {
                    let _ = self.input_tx.send(StateUpdate::new(id.clone(), prop, v.to_string()));
                }
            }
        }
        let form = self.form_object.clone();
        let factor = (layout.font_factor * 100.0).round() / 100.0;
        if self.font_scale_reported != Some(factor) {
            self.font_scale_reported = Some(factor);
            let _ = self.input_tx.send(StateUpdate::new(form.clone(), "FontScale", factor.to_string()));
        }
        let bp = layout.breakpoint.clone();
        match self.breakpoint_reported.replace(bp.clone()) {
            Some(was) if was == bp => {}
            was => {
                let _ = self.input_tx.send(StateUpdate::new(form.clone(), "Breakpoint", bp));
                if was.is_some() {
                    self.send_event(FormEvent::new(form, "onBreakpointChanged"));
                }
            }
        }
    }

    /// Spec 056 R38/R64 — a program's write to a layout property of a
    /// responsive form, carried into the design the layout reads, so the
    /// layout composes with it instead of undoing it on the next pass.
    ///
    /// A geometry value is what the program sees on screen (R37), so it is
    /// turned back into a designed value through the placement the control
    /// was last laid out with (`layout::inverse`): `ADD 10 TO BTN::X` moves a
    /// right-anchored button 10 px on screen at any window size. A container
    /// that moves carries its contents, as a designer drag does. Any other
    /// layout property (`Dock`, `Anchor`, `FontSize`, `Visible`, …) is the
    /// design value itself. The property is marked written, so no breakpoint
    /// overrides it. Content properties are not layout and are left alone.
    fn write_design(&mut self, id: &str, prop: &str, value: &str) {
        let Some(idx) = self.controls.iter().position(|c| c.id.eq_ignore_ascii_case(id)) else { return };
        if !cobolt_forms::layout::breakpoints::overridable(&self.controls[idx], prop) {
            return;
        }
        let lower = prop.to_ascii_lowercase();
        match lower.as_str() {
            "x" | "y" | "width" | "height" => {
                let Some(v) = value.trim().parse::<f32>().ok() else { return };
                let c = &self.controls[idx];
                let designed = cobolt_forms::layout::LRect::from_model(c.rect);
                let layout = self.last_layout.as_ref();
                let laid = layout.and_then(|l| l.rects.get(&c.id)).copied().unwrap_or(designed);
                let placement = layout
                    .and_then(|l| l.placement.get(&c.id))
                    .copied()
                    .unwrap_or(cobolt_forms::layout::Placement::Designed);
                let mut target = laid;
                match lower.as_str() {
                    "x" => target.x = v,
                    "y" => target.y = v,
                    "width" => target.w = v.max(0.0),
                    _ => target.h = v.max(0.0),
                }
                let d = cobolt_forms::layout::inverse::designed_rect(&placement, target, designed);
                let new = cobolt_forms::model::Rect::new(
                    d.x.round() as i32,
                    d.y.round() as i32,
                    d.w.round().max(0.0) as i32,
                    d.h.round().max(0.0) as i32,
                );
                let (dx, dy) = (new.x - self.controls[idx].rect.x, new.y - self.controls[idx].rect.y);
                self.controls[idx].rect = new;
                if dx != 0 || dy != 0 {
                    for d in cobolt_forms::containers::collect_descendants(&self.controls, idx) {
                        self.controls[d].rect.x += dx;
                        self.controls[d].rect.y += dy;
                    }
                }
            }
            "visible" => self.controls[idx].visible = !matches!(value.trim(), "" | "0" | "false" | "FALSE"),
            _ => self.controls[idx].set_prop(prop.to_owned(), cobolt_forms::PropValue::String(value.to_owned())),
        }
        cobolt_forms::layout::breakpoints::mark_written(&mut self.controls[idx], prop);
    }

    /// Carry a COBOL write to a toolbar BUTTON into its toolbar's definition, so
    /// the next frame draws it. Returns whether `id` named a button at all.
    ///
    /// A button's appearance lives in the toolbar's `ToolbarLayout` and nowhere
    /// else, so a live change IS a change to that definition. Rewriting it there
    /// keeps the renderer out of it entirely: it reads the layout off the live
    /// control exactly as it always has.
    ///
    /// The interpreter has already refused anything but a colour or a tooltip and
    /// said so; a write that gets here is one the button allows. A refusal that
    /// still surfaces (a button that has since gone, say) is reported, not
    /// swallowed.
    pub(crate) fn apply_toolbar_button_write(&mut self, id: &str, prop: &str, value: &str) -> bool {
        let Some(found) = cobolt_forms::toolbar::find_button(&self.controls, id) else {
            return false;
        };
        // The designed definition, read the same way the renderer reads it — so a
        // legacy `Items` toolbar is not silently replaced by an empty one.
        let designed = self
            .controls
            .iter()
            .find(|c| c.id == found.toolbar_id)
            .map(cobolt_forms::toolbar::ToolbarDef::from_control)
            .unwrap_or_default();
        // …and whatever a previous write already stored, which wins.
        let live = self.state.get(&found.toolbar_id).and_then(|s| {
            s.props
                .iter()
                .find(|(k, _)| *k == cobolt_forms::toolbar::TOOLBAR_DEF_PROP)
                .map(|(_, v)| v.clone())
        });
        match cobolt_forms::toolbar::write_into_layout(
            &designed,
            live.as_deref(),
            &found.button_id,
            prop,
            value,
        ) {
            Ok(json) => {
                self.state_entry_mut(&found.toolbar_id)
                    .set(cobolt_forms::toolbar::TOOLBAR_DEF_PROP, json);
            }
            Err(refused) => eprintln!("toolbar: {id}::{prop} — {refused}"),
        }
        true
    }

    /// Resolve a control id arriving from COBOL (upper-cased by the compiler)
    /// to the designer's original-case state key — otherwise a handler's
    /// property writes land in an orphan "LABEL-1" entry the renderer (which
    /// looks up by the designed "Label-1" id) never reads.
    pub(crate) fn resolve_ctrl_key(&self, id: &str) -> String {
        if let Some(k) = self.state.keys().find(|k| k.eq_ignore_ascii_case(id)) {
            return k.clone();
        }
        if let Some(c) = self.controls.iter().find(|c| {
            c.explicit_control_array_id()
                .map(|aid| aid.eq_ignore_ascii_case(id))
                .unwrap_or(false)
        }) {
            return c.id.clone();
        }
        id.to_owned()
    }

    /// For a repeating-group member id (case-insensitive), return its
    /// original-case id and the id of its repeating-GroupBox ancestor.
    pub(crate) fn array_member_group(&self, ctrl_id: &str) -> Option<(String, String)> {
        let member = self
            .controls
            .iter()
            .find(|c| c.id.eq_ignore_ascii_case(ctrl_id))?;
        let member_id = member.id.clone();
        let mut cur = member;
        loop {
            let parent_id = cur.parent.as_deref()?;
            let parent = self
                .controls
                .iter()
                .find(|c| c.id.eq_ignore_ascii_case(parent_id))?;
            let is_repeating = matches!(parent.control_type, cobolt_forms::ControlType::GroupBox)
                && parent
                    .get_prop("IsRepeatingGroup")
                    .map(|v| v.as_bool())
                    .unwrap_or(false);
            if is_repeating {
                return Some((member_id, parent.id.clone()));
            }
            cur = parent;
        }
    }

    /// The form's background, resolved once and shared by the live render and
    /// by the static face the window effects animate — so an entrance reveals
    /// the form WITH its gradient / background image instead of jumping to it
    /// when the animation ends.
    ///
    /// `extent` is the size of the SURFACE this backdrop is laid out against —
    /// the window for a form that owns one, the PANE for a form loaded into a
    /// ContentPane. It is not always `ctx.content_rect()`: an occupant is
    /// drawn into a sub-rect of the shell window, and laying its backdrop out
    /// against the whole window made every aspect-preserving mode wrong (see
    /// `window_size` below).
    pub(crate) fn backdrop(
        &self,
        ctx: &egui::Context,
        extent: egui::Vec2,
    ) -> cobolt_forms::render::Backdrop {
        // Background image texture (cached in egui memory by path).
        let image = if self.bg_image.trim().is_empty() {
            None
        } else {
            let path = self.bg_image.clone();
            let id = egui::Id::new(("form_host_bg", path.as_str()));
            let cached = ctx.memory(|m| m.data.get_temp::<Option<egui::TextureHandle>>(id));
            let tex = match cached {
                Some(t) => t,
                None => {
                    let loaded = cobolt_forms::paint::load_image_texture(ctx, &path);
                    ctx.memory_mut(|m| m.data.insert_temp(id, loaded.clone()));
                    loaded
                }
            };
            tex.map(|t| (t.id(), t.size_vec2()))
        };
        // Under a see-through theme (Spatial, spec 083) the backdrop is the
        // theme's translucent glass: a form's own solid colour, gradient or
        // picture would hide the blurred desktop the theme exists to show.
        let see_through = self.surface_theme.see_through();
        let (image, color_hex) = match see_through
            .then(|| self.surface_theme.token(cobolt_forms::surface_theme::ColorToken::FormBackground))
            .flatten()
        {
            Some(glass) => {
                let [r, g, b, a] = glass.to_srgba_unmultiplied();
                (None, format!("#{r:02X}{g:02X}{b:02X}{a:02X}"))
            }
            None => (image, self.bg_hex.clone()),
        };
        cobolt_forms::render::Backdrop {
            paint: true,
            color_hex,
            transparency: self.transparency,
            gradient_enabled: self.bg_gradient_enabled && !see_through,
            gradient_start_hex: self.bg_gradient_start.clone(),
            gradient_end_hex: self.bg_gradient_end.clone(),
            gradient_direction: self.bg_gradient_direction.clone(),
            image,
            image_mode: self.bg_mode,
            use_theme_background: self.use_theme_background,
            // The gradient / background image follows the SURFACE: it
            // stretches over the whole thing when the user maximizes or drags
            // it bigger, and stays form-sized when the surface is smaller. The
            // controls keep their designed size either way.
            //
            // The surface is NOT always the window. A form loaded into the
            // shell's ContentPane occupies a sub-rect of it — narrower by the
            // MenuPane rail, shorter by the breadcrumb band — and this used to
            // read `ctx.content_rect()` regardless. Every mode is evaluated
            // against this extent, so Fit letterboxed against the WINDOW and
            // put its bars outside the pane, Fill and Center centred on the
            // window's midpoint rather than the pane's: an embedded form
            // showed the same edge-to-edge crop under Fit, Fill and Stretch
            // alike, and the picture slid as the window resized (operator,
            // 2026-08-31: "fit/stretched seems to be the same").
            window_size: Some(extent),
            // A window form paints its OWN backdrop, so `bg` already answers
            // "what is behind" for the corner-notch mask. The one case it does
            // not — a see-through window, where the desktop is behind — has no
            // honest colour to state, and the rounded-clip path is what fixes
            // that properly. The pane is where this matters, and the pane sets
            // it: see the `Surface::Pane` branch in `ui_impl`.
            behind_fill: None,
            image_extent: None,
            draggable: self.owns_window && !self.title_visible,
            window: self.window_arc(ctx),
        }
    }

    /// The rounded window this body's backdrop and top-level controls are cut
    /// to this frame: its own viewport when it owns a window without a title
    /// bar, or the window a shell draws it into (`pane_window`).
    pub(crate) fn window_arc(&self, ctx: &egui::Context) -> Option<cobolt_forms::paint::ContainerClip> {
        if self.owns_window {
            if self.title_visible {
                return None;
            }
            return cobolt_forms::render::window_arc(ctx.content_rect(), self.corner_radius);
        }
        self.pane_window
    }

    /// 051 Q2 (operator ruling) — tick this PARKED form's enabled Timer
    /// controls: off-pane, render-driven timers stand still, so the host
    /// fires `onTick` from its own clocks (with the usual backlog
    /// coalescing) and timer handlers keep running. Returns the earliest
    /// next-due delay, for `request_repaint_after`.
    pub(crate) fn tick_parked_timers(&mut self) -> Option<std::time::Duration> {
        let now = std::time::Instant::now();
        let mut next: Option<std::time::Duration> = None;
        let timers: Vec<(String, u64)> = self
            .controls
            .iter()
            .filter(|c| c.control_type == cobolt_forms::ControlType::Timer)
            .filter_map(|c| {
                let cs = self.state.get(&c.id)?;
                if !cs.enabled {
                    return None;
                }
                let interval = cs
                    .props
                    .iter()
                    .find(|(k, _)| k.eq_ignore_ascii_case("Interval"))
                    .and_then(|(_, v)| v.trim().parse::<u64>().ok())
                    .unwrap_or(1000)
                    .max(16);
                Some((c.id.clone(), interval))
            })
            .collect();
        // While the form renders, the render engine owns the clocks — reset
        // ours on the way back to parked so a stale epoch cannot fire a
        // burst of catch-up ticks.
        for (id, interval) in timers {
            let clock = self
                .parked_timer_clocks
                .entry(id.clone())
                .or_insert(now);
            let elapsed = now.duration_since(*clock);
            let period = std::time::Duration::from_millis(interval);
            if elapsed >= period {
                *clock = now;
                // WinForms-style coalescing: a queued backlog swallows ticks.
                if self.pending.load(Ordering::Relaxed) == 0 {
                    self.send_event(FormEvent::new(id, "onTick"));
                }
                next = Some(next.map_or(period, |n: std::time::Duration| n.min(period)));
            } else {
                let due = period - elapsed;
                next = Some(next.map_or(due, |n: std::time::Duration| n.min(due)));
            }
        }
        next
    }

    /// 051 — one frame of a CHILD window: drain, lifecycle, render, forward.
    /// The compact sibling of the root's `ui_impl` — no window effects, no
    /// pane, no supervisor (the parent host owns those); everything a live
    /// form needs, through the same shared render engine. `blocked` disables
    /// input while the child's own modal child lives (R28).
    /// This body's SideMenu, when it has one — the rail that owns a breadcrumb.
    fn own_side_menu(&self) -> Option<&cobolt_forms::Control> {
        cobolt_forms::breadcrumb::shell_side_menu_in(&self.controls)
    }

    /// The controls this body PAINTS: the design, except that a rail shown
    /// collapsed is drawn at its collapsed width, with the content slid over
    /// the column it gave up — `sidebar::rail_view`, the call the designer
    /// canvas and the preview already make.
    ///
    /// The root form never needed it: it opens as a SHELL, which lays the rail
    /// out itself. A child window did — it painted the designed rect, so a
    /// folded rail was a full-width bar of icon-only rows, and the breadcrumb
    /// strip (placed from the COLLAPSED width by `window_crumb_chrome`) sat
    /// underneath it (operator, 2026-09-23).
    fn painted_controls(&self) -> Vec<cobolt_forms::Control> {
        match self.own_side_menu() {
            Some(side) => {
                let collapsed = self.side_menu_collapsed(&side.id);
                cobolt_forms::sidebar::rail_view(&self.controls, side, collapsed)
            }
            None => self.controls.clone(),
        }
    }

    /// The rail's live Open/Collapsed state, which is what the toggle's arrow
    /// has to show: the designed property only says what it opened as.
    fn side_menu_collapsed(&self, side_id: &str) -> bool {
        match self.state.get(side_id).and_then(|s| s.props.get("Collapsed")) {
            Some(v) => matches!(v.trim(), "1" | "true" | "True" | "TRUE"),
            None => self
                .own_side_menu()
                .map(|c| c.side_menu_collapsed())
                .unwrap_or(false),
        }
    }

    /// The breadcrumb strip a form running in its OWN WINDOW draws for itself.
    ///
    /// A form carrying a SideMenu opens as a SHELL when it is the root — that
    /// is what `rcrun run-form` and a built application do — and the shell's
    /// breadcrumb carries the rail's Open/Collapsed control at its head. The
    /// designer canvas and the preview draw that same strip, so it is the
    /// control the developer designs against.
    ///
    /// Spawned as a CHILD WINDOW by another form's sidebar, the very same form
    /// was a bare viewport with no shell over it (`child_frame` is handed
    /// `None` for its chrome), so the control had nowhere to live: the rail
    /// could still be folded by clicking its header, but there was nothing to
    /// see or aim at. The operator reported it as the sidebar losing its
    /// fold/unfold button when the form is launched from another sidebar
    /// window (2026-09-10).
    ///
    /// The chain is ONE STATIC SEGMENT — this form — for the reason the design
    /// surfaces give: a navigation chain is a runtime fact of the SHELL, and a
    /// child window is not in one, so there is nothing else to honestly show.
    /// The toggle is live.
    ///
    /// Returns the painter `child_frame` runs between the backdrop and the
    /// controls, so the strip sits under anything the developer placed over the
    /// band — exactly where the designer canvas puts it.
    /// `label` is what the chain calls this form — the window title the
    /// child was opened with, which is already the designed Title with the
    /// form object as its fallback.
    pub(crate) fn window_crumb_chrome(
        &mut self,
        ui: &egui::Ui,
        window: egui::Rect,
        label: &str,
    ) -> Option<Box<dyn Fn(&egui::Painter, egui::Rect)>> {
        use cobolt_forms::breadcrumb as bc;
        let side = self.own_side_menu()?.clone();
        let collapsed = self.side_menu_collapsed(&side.id);
        let rail = cobolt_forms::sidebar::shown_width(&side, collapsed);
        let rect = bc::strip_rect(&side, rail, window.width(), window.min)?;

        let ctx = ui.ctx().clone();
        let bg = bc::strip_background_for(&side, &self.bg_hex, self.transparency);
        let segments = vec![if label.trim().is_empty() {
            self.form_name.clone()
        } else {
            label.to_owned()
        }];
        let mut state = bc::state_for_control(&ctx, &side, &segments, bg);
        state.collapsed = collapsed;
        let layout = bc::layout(ui.painter(), rect, &state);
        state.toggle_hovered = ctx
            .pointer_interact_pos()
            .is_some_and(|p| bc::toggle_hit(&layout, p));
        self.last_window_crumb = Some(layout.clone());

        // The state borrows `segments`, so both travel into the closure.
        let side_for_paint = side.clone();
        let collapsed_for_paint = collapsed;
        let hovered = state.toggle_hovered;
        Some(Box::new(move |painter: &egui::Painter, _pane: egui::Rect| {
            let mut st = bc::state_for_control(&ctx, &side_for_paint, &segments, bg);
            st.collapsed = collapsed_for_paint;
            st.toggle_hovered = hovered;
            bc::paint(painter, rect, &st, &layout);
        }))
    }

    /// Register the window strip's toggle against what it laid out this frame,
    /// and fold or unfold the rail when it is clicked.
    ///
    /// Registered BEFORE the form's controls, so a control the developer placed
    /// over the band still wins the pointer: the strip is chrome, and chrome
    /// never steals a click from the developer's own control.
    pub(crate) fn window_crumb_interact(&mut self, ui: &mut egui::Ui) {
        let Some(layout) = self.last_window_crumb.clone() else {
            return;
        };
        let Some(side_id) = self.own_side_menu().map(|c| c.id.clone()) else {
            return;
        };
        if !ui
            .interact(
                layout.toggle,
                ui.id().with(("window-crumb-toggle", &side_id)),
                egui::Sense::click(),
            )
            .clicked()
        {
            return;
        }
        // The same write the rail's own header click makes, through the same
        // door, so the two affordances cannot disagree and the COBOL handler
        // hears about either one.
        let collapsed = self.side_menu_collapsed(&side_id);
        let next = if collapsed { "0" } else { "1" };
        self.forward_interaction(
            &[(side_id.clone(), "Collapsed".to_owned(), next.to_owned())],
            vec![cobolt_forms::render::UiEvent {
                ctrl_id: side_id.clone(),
                event: if collapsed { "onMenuOpen" } else { "onMenuClose" }.to_owned(),
                value: None,
            }],
            // `update_children` only calls this while NOT blocked.
            false,
        );
    }

    pub(crate) fn child_frame(
        &mut self,
        panel_ui: &mut egui::Ui,
        blocked: bool,
        // Chrome painted between this form's backdrop and its controls — the
        // shell's breadcrumb frame when this body is the ContentPane occupant.
        // `None` for a child window, which has no shell chrome over it.
        chrome: Option<cobolt_forms::render::ChromeUnderControls<'_>>,
    ) {
        // Panels are Ui-hosted since egui 0.35; everything else here wants a
        // Context.
        let ctx = panel_ui.ctx().clone();
        let ctx = &ctx;
        // The surface this body is drawn into, taken BEFORE the Ui is handed
        // to the CentralPanel. For a child window it is the viewport; for the
        // ContentPane occupant it is the pane rect the shell carved out. The
        // backdrop is laid out against this — see `FormBody::backdrop`.
        let panel_extent = panel_ui.max_rect().size();
        // The pane's real screen rect, ORIGIN included: comparing drawn rects
        // against a rect at (0,0) reports every control off-surface and means
        // nothing.
        let panel_rect = panel_ui.max_rect();
        // A child window's controls live in an id space of their own, or two
        // forms that both have a `Btn-Play` claim one widget id, take each
        // other's clicks and focus — and share each other's state: every form
        // here has a `Tmr-Lang`, a timer keeps its last tick under its id, and
        // windows sharing one id space shared ONE timer, so only one of them
        // ticked each second (operator, 2026-10-04: "the bottom form is not
        // changing the language"). The space is the window's own: its
        // viewport when it is a real window, and the layer it is drawn on
        // when every window is embedded in one viewport, as the headless host
        // draws them. Both are stable from frame to frame.
        let scope = self
            .owns_window
            .then(|| child_window_scope(ctx.viewport_id(), panel_ui.layer_id()));
        // Theme state for the unified painter — this viewport's own context.
        cobolt_forms::paint::set_active_theme(ctx, self.theme_pack.clone());
        cobolt_forms::paint::set_glass_style(ctx, self.glass_style);
        cobolt_forms::paint::set_surface_theme(ctx, self.surface_theme.clone());
        self.surface_theme.install_widget_visuals(ctx);

        // Animation clock.
        let now = std::time::Instant::now();
        let dt = self
            .last_frame
            .map(|t| now.duration_since(t).as_secs_f32())
            .unwrap_or(0.0);
        self.last_frame = Some(now);
        let animating = self.anim.tick(dt);

        // Interpreter → UI property updates (the root's routing rules).
        let updates: Vec<StateUpdate> = self.state_rx.try_iter().collect();
        let drained = updates.len();
        for u in updates {
            // A child WINDOW's own window; a ContentPane occupant has none.
            let vp = ctx.viewport_id();
            let own_window = (vp != egui::ViewportId::ROOT).then_some(vp);
            self.apply_form_window_update(ctx, &u, own_window);
            // A child form rides the same diagnostics switch the root does; the
            // host reads it once at start-up, a body reads it here.
            self.apply_interpreter_update(u, crate::diagnostics::frame_diagnostics_enabled());
        }

        // 058 R18.1 — a Save As the drain above asked for, and the answer to
        // one asked for earlier. Both here, next to the drain that raises them,
        // because a native dialog is non-blocking: it is opened on one frame
        // and answered on some later one.
        self.start_pending_save_as(ctx);
        self.collect_viewer_save_as();
        // R19/R20 — the same shape: started on one frame, answered on a later
        // one, because a print spooler takes as long as it takes.
        self.drive_viewer_os_handoffs(ctx);

        // DISPLAY → stdout (the IDE's Output pane reads it there).
        {
            let mut any = false;
            while let Ok(line) = self.display_rx.try_recv() {
                println!("{line}");
                any = true;
            }
            if any {
                use std::io::Write;
                let _ = std::io::stdout().flush();
            }
        }

        // Warm-up, then the form-level lifecycle pair — exactly once.
        let armed = self.start.elapsed().as_millis() > 450;
        if armed && !self.lifecycle_sent {
            self.lifecycle_sent = true;
            let name = self.form_name.clone();
            // The catalogue's Lifecycle order: onLoad (a direct CALL in the
            // generated program, already done by now), then onOpened, then
            // onShow. `onActivated` is the past-tense twin of `onActivate` and
            // follows it — both are offered in the designer, and only the
            // present-tense one used to fire.
            self.send_event(FormEvent::new(&name, "onOpened"));
            self.send_event(FormEvent::new(&name, "onShow"));
            self.send_event(FormEvent::new(&name, "onActivate"));
            self.send_event(FormEvent::new(&name, "onActivated"));
        }

        // A see-through (rounded) child window fills nothing: the engine's
        // rounded backdrop is its only paint, so the desktop shows past the arc.
        let bg_fill = if self.see_through_window {
            egui::Color32::TRANSPARENT
        } else {
            cobolt_forms::render::backdrop_color(&self.bg_hex, self.transparency)
        };
        let form_size = self.form_size;
        // Focus as it stood BEFORE this frame's widgets see the click — the
        // click that presses a toolbar button surrenders the text field's
        // focus during render, and the clipboard verbs need to know who HAD it.
        let pre_focus = ctx.memory(|m| m.focused());
        let mut laid_layout: Option<cobolt_forms::layout::LayoutOutput> = None;
        let output = {
            let mut controls = self.painted_controls();
            // 056 — a responsive form lays out from its DESIGNED controls, with
            // the rail's shown state applied afterwards (R26).
            let responsive = self.responsive.clone();
            let designed_controls = if responsive.is_some() { self.controls.clone() } else { Vec::new() };
            let rail = self
                .own_side_menu()
                .map(|side| (side.id.clone(), self.side_menu_collapsed(&side.id)));
            let st = LiveState {
                state: &self.state,
                anim: &self.anim,
                hidden: Some(&self.footer_ids),
                viewer_docs: None,
                special_names: &self.special_names,
            };
            let active_tabs = self.active_tabs();
            // The surface is the Ui handed to us, never the window: for a
            // child WINDOW that is the viewport (unchanged), for the
            // ContentPane occupant it is the pane.
            let backdrop = self.backdrop(ctx, panel_extent);
            let mut out = cobolt_forms::render::RenderOutput::default();
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE.fill(bg_fill))
                .show(panel_ui, |ui| {
                    if blocked {
                        ui.disable();
                        // `disable()` also multiplied the painter's opacity by
                        // `disabled_alpha` (0.5) — and a ContentPane occupant
                        // inherits the shell's own halving on top of that, so
                        // a blocked form drew at a quarter of its designed
                        // strength and read as "greyed" whatever
                        // ModalOverlayStyle said. The form keeps exactly the
                        // transparency it was designed with; the overlay
                        // below is the ONLY dimming (operator, 2026-09-19).
                        ui.set_opacity(1.0);
                    }
                    ui.style_mut().spacing.scroll = form_scroll_style();
                    // 056 R23 — the surface a responsive form lays out for,
                    // held inside the form's size limits (R18). A window gets
                    // those limits from the OS; a form loaded into a
                    // ContentPane has no window to stop, so the pane grew it
                    // until its controls ran into each other (operator,
                    // 2026-10-02). Past the maximum the pane's extra space
                    // stays empty; below the minimum the scroll area scrolls.
                    // In a child window this changes nothing: the OS already
                    // held it there.
                    let surface_size = match responsive.as_ref() {
                        Some(spec) => {
                            let (min, max) = spec.size_limits(&designed_controls, form_size);
                            ui.available_size().max(min).min(max)
                        }
                        None => ui.available_size(),
                    };
                    egui::ScrollArea::both()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            let laid_state = cobolt_forms::layout::apply::LaidOutState { inner: &st };
                            // R26 — lay out the DESIGNED controls first; the
                            // SideMenu rail then narrows on the laid-out rects,
                            // as it narrows on designed ones today.
                            let prepared = responsive.as_ref().map(|spec| {
                                let rail = rail.as_ref().map(|(id, collapsed)| (id.as_str(), *collapsed));
                                spec.prepare(ui.ctx(), &designed_controls, &st, form_size, surface_size, rail)
                            });
                            laid_layout = prepared.as_ref().map(|p| p.layout.clone());
                            let (render_controls, render_size, render_state): (
                                &[cobolt_forms::Control],
                                egui::Vec2,
                                &dyn cobolt_forms::render::FormState,
                            ) = match &prepared {
                                // R43 — the bars follow their anchors.
                                Some(p) => (&p.controls, p.form_size, &laid_state),
                                None => {
                                    // A Responsive MenuBar and a StatusBar span
                                    // the window the operator sees, not only
                                    // the width the form was designed at.
                                    cobolt_forms::Form::stretch_window_bars(
                                        &mut controls,
                                        ui.available_width().max(form_size.x),
                                    );
                                    (&controls, form_size, &st)
                                }
                            };
                            ui.set_min_size(render_size);
                            let input = cobolt_forms::render::RenderInput {
                                controls: render_controls,
                                state: render_state,
                                form_size: render_size,
                                glass: true,
                                mode: cobolt_forms::render::RenderMode::Interactive,
                                active_tabs: &active_tabs,
                                backdrop,
                            };
                            out = match scope {
                                Some(s) => cobolt_forms::render::render_form_scoped(ui, &input, chrome, s),
                                None => cobolt_forms::render::render_form_with_chrome(ui, &input, chrome),
                            };
                        });
                });
            // A child window without a title bar moves by its face. Inside
            // `show_viewport_immediate` the context's viewport IS this window.
            if out.window_drag && self.owns_window {
                ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
            }
            // 051 R19/R28 — `disable()` above already refuses input; this is
            // only the paint that lets the operator SEE this form is waiting,
            // in the style ITS OWN design chose (a ContentPane occupant is
            // its own form here, not the shell — the shell has no `.cfrm` of
            // its own to carry the choice).
            if let Some(fill) = if blocked {
                modal_overlay_fill(self.modal_overlay_style)
            } else {
                None
            } {
                // Through a FRESH painter, not `panel_ui.painter()`: the
                // shell disables its whole root `Ui` while blocked
                // (`ShellApp::ui`), and egui's `Ui::disable` multiplies the
                // painter's opacity by `disabled_alpha` (0.5) — which every
                // child `Ui` inherits. Painted through the inherited painter
                // the overlay itself arrived at half its alpha (Greyed's 150
                // → 75), so under the sidebar both styles read as the same
                // faint wash, while a form run standalone (whose root `Ui`
                // is never disabled) showed the real fill (operator report,
                // 2026-09-19). The overlay IS the "you are waiting" signal;
                // it must paint at its designed strength.
                // …and cut to a rounded window's arc, like everything else.
                cobolt_forms::paint::fill_in_clip(&overlay_painter(panel_ui), panel_rect, fill, self.window_arc(ctx));
            }
            out
        };
        // Where the engine actually put every control this frame — see
        // `FormBody::last_control_rects`.
        self.last_control_rects = output.control_rects.clone();
        self.last_layout = laid_layout;
        self.mirror_layout();

        // 055 — notifications, over the controls and inside THIS body's pane
        // (D3/R16). `panel_rect` is the pane the shell carved out for an
        // Embedded occupant and the viewport for a child window, so a message
        // lands where the operator is looking and never over the rail or the
        // breadcrumb.
        self.draw_snackbars(panel_ui, panel_rect);

        // …and, once the entrance has settled, say so out loud. A control that
        // arrives visible at its designed rect and still does not appear is
        // being drawn somewhere unexpected, and nothing reported that.
        if !self.drawn_reported
            && !self.last_control_rects.is_empty()
            && crate::diagnostics::frame_diagnostics_enabled()
        {
            self.drawn_reported = true;
            crate::diagnostics::drawn_rects(
                &self.form_name,
                panel_rect,
                &self.controls,
                &self.last_control_rects,
            );
        }
        let mut platform_acted = false;
        if armed && !blocked {
            // Animation triggers from this frame's interaction — the root's
            // rect-derived hover/click rules.
            let (clicked, pointer) =
                ctx.input(|i| (i.pointer.primary_clicked(), i.pointer.interact_pos()));
            let mut still_hovered = std::collections::HashSet::new();
            for (id, rect) in &output.control_rects {
                if id.contains('.') {
                    continue;
                }
                let over = pointer.map(|p| rect.contains(p)).unwrap_or(false);
                if over {
                    still_hovered.insert(id.clone());
                    if !self.hovered.contains(id) {
                        self.anim.fire_event(&self.controls, id, "onHoverEnter");
                    }
                    if clicked {
                        self.anim.fire_event(&self.controls, id, "onClick");
                    }
                }
            }
            self.hovered = still_hovered;
            for ev in &output.events {
                if ev.event.eq_ignore_ascii_case("onClick")
                    || ev.event.eq_ignore_ascii_case("onHoverEnter")
                {
                    continue;
                }
                self.anim.fire_event(&self.controls, &ev.ctrl_id, &ev.event);
            }

            // Live values to the interpreter, then the events — with the
            // timer-tick backlog coalescing. Shared with the root's path.
            if self.forward_interaction(&output.prop_updates, output.events, blocked) {
                platform_acted = true;
            }

            // A FileDropZone's native picker and a toolbar button's platform
            // action. This ran on the ROOT form only, so a toolbar in a child
            // window or a ContentPane occupant had eight dead actions and its
            // FileDropZone would not open a picker.
            if self.run_platform_requests(
                ctx,
                &output.file_picker_requests,
                &output.csv_export_requests,
                &output.toolbar_actions,
                pre_focus,
                scope,
            ) {
                platform_acted = true;

            }
        }
        self.show_action_notice(ctx);

        // A busy child keeps frames coming; an idle one rides the root's
        // heartbeat.
        if drained > 0 || platform_acted || animating || self.anim.is_animating() {
            ctx.request_repaint_after(std::time::Duration::from_millis(16));
        }
    }

    /// Opt-in diagnostic (`COBOLT_DATABIND_TRACE=1`). For each repeating-group
    /// member of instance 1, write the exact id the renderer looks up and whether
    /// that id is present in `state` byte-exact vs. case-insensitively. A CI-only
    /// hit means the value landed under a differently-cased key than the render
    /// draws with — the classic run-form databind blank. Written once to
    /// `cobolt-databind-render.log` in the platform's diagnostics directory.
    pub(crate) fn dump_databind_trace(&self) {
        use std::io::Write;
        let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(cobolt_runtime::diag_path::diagnostics_file(
                "cobolt-databind-render.log",
            ))
        else {
            return;
        };
        let _ = writeln!(
            f,
            "\n=== RENDER-SIDE DATABIND TRACE ({}) ===",
            self.form_name
        );
        let inst_keys = self.state.keys().filter(|k| k.contains('.')).count();
        let _ = writeln!(f, "state has {inst_keys} instanced ('.') keys total");
        for g in &self.controls {
            let is_rep = matches!(g.control_type, cobolt_forms::ControlType::GroupBox)
                && g.get_prop("IsRepeatingGroup")
                    .map(|v| v.as_bool())
                    .unwrap_or(false);
            if !is_rep {
                continue;
            }
            let members: Vec<&cobolt_forms::Control> = self
                .controls
                .iter()
                .filter(|c| {
                    c.parent
                        .as_deref()
                        .map(|p| p.eq_ignore_ascii_case(&g.id))
                        .unwrap_or(false)
                })
                .collect();
            let _ = writeln!(
                f,
                "group '{}' members=[{}]",
                g.id,
                members
                    .iter()
                    .map(|m| m.id.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            for m in &members {
                let id = cobolt_forms::render::member_instance_id(&g.id, &m.id, 1);
                let exact = self.state.contains_key(&id);
                let ci = self
                    .state
                    .keys()
                    .find(|k| k.eq_ignore_ascii_case(&id))
                    .cloned();
                let _ = writeln!(
                    f,
                    "  lookup '{id}' -> exact={exact} ci_key={:?}",
                    ci.filter(|k| *k != id)
                );
            }
        }
        let _ = writeln!(f, "sample instanced keys:");
        for k in self.state.keys().filter(|k| k.contains('.')).take(8) {
            let _ = writeln!(f, "  {k}");
        }
    }
}

/// Spec 085 — a child window that runs as a shell, as its shell sees it: the
/// window's own form and its ContentPane.
struct ChildPaneView<'a> {
    body: &'a mut FormBody,
    pane: &'a mut Pane,
    blocked: bool,
    overlay: Option<cobolt_forms::model::ModalOverlayStyle>,
}

impl crate::shell::PaneHost for ChildPaneView<'_> {
    fn publish_root_theme(&self, ctx: &egui::Context) {
        cobolt_forms::paint::set_active_theme(ctx, self.body.theme_pack.clone());
        cobolt_forms::paint::set_glass_style(ctx, self.body.glass_style);
        cobolt_forms::paint::set_surface_theme(ctx, self.body.surface_theme.clone());
    }

    fn control_prop(&self, ctrl_id: &str, prop: &str) -> Option<String> {
        let key = self.body.resolve_ctrl_key(ctrl_id);
        self.body.state.get(&key).and_then(|s| {
            s.props.iter().find(|(k, _)| k.eq_ignore_ascii_case(prop)).map(|(_, v)| v.clone())
        })
    }

    fn draw_side_menu_footer(&mut self, ui: &mut egui::Ui, band: egui::Rect, behind: egui::Color32) {
        self.body.draw_side_menu_footer(ui, band, behind, self.blocked);
    }

    fn set_pane_chrome(&mut self, chrome: Option<Box<dyn Fn(&egui::Painter, egui::Rect)>>, band: f32) {
        self.pane.pane_chrome = chrome;
        self.pane.pane_band = band;
    }

    fn pane_frame(&mut self, pane_ui: &mut egui::Ui) {
        let rect = pane_ui.available_rect_before_wrap();
        let chrome = self.pane.pane_chrome.take();
        let band = self.pane.pane_band;
        // A form loaded into the pane starts BELOW the breadcrumb band (it has
        // its own coordinate space); the window's own form may design
        // controls over the band — the same rule as the main window.
        // A shell in a child window: that window is the child's own, rounded
        // by the child's form; the pane occupant is a piece of it.
        let arc = self.body.window_arc(pane_ui.ctx());
        if let Some(occ) = self.pane.active_occupant.as_ref().and_then(|k| self.pane.occupants.get_mut(k)) {
            occ.body.pane_window = arc;
            if let Some(chrome) = chrome.as_deref() {
                chrome(pane_ui.painter(), rect);
            }
            let mut inner = rect;
            inner.min.y += band;
            self.pane.last_occupant_rect = Some(inner);
            let mut ui = pane_ui.new_child(egui::UiBuilder::new().max_rect(inner));
            occ.body.child_frame(&mut ui, self.blocked, None);
        } else {
            self.pane.last_occupant_rect = None;
            let mut ui = pane_ui.new_child(egui::UiBuilder::new().max_rect(rect));
            self.body.child_frame(&mut ui, self.blocked, chrome.as_deref());
        }
    }

    fn blocked_overlay_style(&self) -> Option<cobolt_forms::model::ModalOverlayStyle> {
        self.overlay
    }
}

/// 049 — a form shown in a shell's ContentPane: the SideMenu IS the shell's
/// MenuPane, painted as chrome outside the form, so its control is dropped
/// from the paint; the content slides over the rail's DESIGNED column so its
/// left edge lands on the pane's (a control parked under the rail clamps to
/// the edge); and the SideMenu's footer Panel subtree keeps its designed
/// rects — it is rail, drawn into the rail's footer band. Returns the
/// controls, the footer subtree's ids and the rail's designed width.
///
/// The main window's form (`FormHost::new` on the Pane surface) and a child
/// window that runs as a shell (spec 085) take exactly this transform.
pub(crate) fn pane_layout(
    flat: Vec<cobolt_forms::Control>,
) -> (Vec<cobolt_forms::Control>, std::collections::HashSet<String>, i32) {
    let side_dx = flat
        .iter()
        .find(|c| c.control_type == cobolt_forms::ControlType::SideMenu)
        .map(|c| c.rect.w.max(0))
        .unwrap_or(0);
    let footer_ids: std::collections::HashSet<String> =
        cobolt_forms::model::side_menu_footer_subtree(&flat).into_iter().collect();
    let flat = flat
        .into_iter()
        .filter(|c| c.control_type != cobolt_forms::ControlType::SideMenu)
        .map(|mut c| {
            if !footer_ids.contains(&c.id) {
                c.rect.x = (c.rect.x - side_dx).max(0);
            }
            c
        })
        .collect();
    (flat, footer_ids, side_dx)
}

/// Keep a docked child against its opener (form `DockToOpener`, operator
/// 2026-10-04). The window is placed — and sized, for `DockLength` — only when
/// the opener's window has moved or changed size since the window was last
/// placed, once per change. A docked window never moves anything itself: it
/// follows its opener, and nothing follows it.
///
/// It used to be put back every frame, and a docked window found anywhere but
/// where it was last put was taken to have been dragged, so the opener was
/// moved after it. The operating system applies a move a frame or more later
/// and keeps a window inside the screen, so a position read back early, or
/// clamped, looked like a drag: the bars pushed the main window, the main
/// window pulled the other bars, and the stream of moves kept every window
/// from taking the focus ("the forms are fighting each other").
fn dock_child_window(ctx: &egui::Context, child: &mut ChildWindow, opener_vp: egui::ViewportId) {
    let opener = ctx.input_for(opener_vp, |i| i.viewport().outer_rect);
    let mine = ctx.input_for(child.viewport_id, |i| i.viewport().outer_rect);
    let (Some(o), Some(m)) = (opener, mine) else { return };
    // Where the platform has window groups (macOS), the docked window is made
    // a native child of its opener: the operating system then moves the two
    // in the same screen update, while the opener is dragged, instead of this
    // host chasing it a frame behind — the jagged edge between a moving
    // dashboard and its bars. Tried each frame until it takes.
    // The native calls take screen points; egui's are those over the zoom
    // (`fit_window_group` zooms a group too large for the screen).
    let zoom = ctx.zoom_factor() as f64;
    let rect = |r: egui::Rect| {
        (r.min.x as f64 * zoom, r.min.y as f64 * zoom, r.width() as f64 * zoom, r.height() as f64 * zoom)
    };
    if !child.dock_attached {
        child.dock_attached = cobolt_os_blur::attach_child_window(rect(o), rect(m));
    }
    // A click on a docked window brings its whole group forward: the opener
    // is ordered front, and its attached windows with it — the window clicked
    // keeps the keyboard (operator, 2026-10-04: "a click in a single form
    // brings all of them to the front"). Once per gain of focus.
    let focused = ctx.input_for(child.viewport_id, |i| i.viewport().focused).unwrap_or(false);
    if focused && !child.dock_focused {
        cobolt_os_blur::raise_group(rect(o));
    }
    child.dock_focused = focused;
    if !opener_changed(child.dock_opener_seen, o) {
        return;
    }
    // `DockLength`: the window's length along the edge follows the opener's.
    // Its inner size is what the viewport is declared with each frame, so
    // changing it is the resize; the place below is worked out for that size.
    let inner = ctx.input_for(child.viewport_id, |i| i.viewport().inner_rect).map_or(m.size(), |r| r.size());
    let frame = ((m.width() - inner.x).max(0.0), (m.height() - inner.y).max(0.0));
    let mut outer = m.size();
    if let Some((w, h)) = cobolt_forms::model::dock_size(
        child.dock,
        child.dock_length,
        (o.width(), o.height()),
        (child.size.x, child.size.y),
        frame,
    ) {
        child.size = egui::vec2(w, h);
        outer = egui::vec2(w + frame.0, h + frame.1);
    }
    if let Some((x, y)) = cobolt_forms::model::dock_position(
        child.dock,
        child.dock_gap,
        (o.min.x, o.min.y, o.width(), o.height()),
        (outer.x, outer.y),
    ) {
        ctx.send_viewport_cmd_to(child.viewport_id, egui::ViewportCommand::OuterPosition(egui::pos2(x, y)));
    }
    child.dock_opener_seen = Some(o);
}

/// What a fit of the main window's group was made for
/// ([`FormHost::fit_window_group`]): the usable screen, rounded, and each
/// docked window's edge, gap and thickness.
type GroupFitKey = ([i32; 4], Vec<(cobolt_forms::model::DockEdge, i32, i32)>);

/// The id space a child window's controls are rendered in: its viewport, for a
/// real window, and its layer, for one embedded in another's viewport.
fn child_window_scope(viewport: egui::ViewportId, layer: egui::LayerId) -> egui::Id {
    egui::Id::new("form-window").with(viewport).with(layer)
}

/// Whether the opener's window is somewhere else, or another size, than when
/// its docked window was last placed (`seen`) — the one thing that moves a
/// docked window. Half a point of drift is the same place.
fn opener_changed(seen: Option<egui::Rect>, now: egui::Rect) -> bool {
    seen.is_none_or(|r| (r.min - now.min).length() > 0.5 || (r.size() - now.size()).length() > 0.5)
}

#[cfg(test)]
mod dock_tests {
    use super::*;

    /// Every child window has an id space of its own: two real windows draw on
    /// the same (background) layer of their own viewports, and two embedded
    /// ones on their own layers of the same viewport — either way the spaces
    /// differ, so two forms' same-named controls (a `Tmr-Lang` in each) never
    /// share state.
    #[test]
    fn each_child_window_has_its_own_id_space() {
        let background = egui::LayerId::background();
        let a = egui::ViewportId::from_hash_of("a");
        let b = egui::ViewportId::from_hash_of("b");
        assert_ne!(child_window_scope(a, background), child_window_scope(b, background), "two real windows");
        let la = egui::LayerId::new(egui::Order::Middle, egui::Id::new("la"));
        let lb = egui::LayerId::new(egui::Order::Middle, egui::Id::new("lb"));
        let root = egui::ViewportId::ROOT;
        assert_ne!(child_window_scope(root, la), child_window_scope(root, lb), "two embedded windows");
        assert_eq!(child_window_scope(a, background), child_window_scope(a, background), "stable");
    }

    /// A docked window is placed the first time, and again only when its
    /// opener moves or resizes — never because of where the window itself was
    /// found, so it cannot push its opener or chase the operating system.
    #[test]
    fn a_docked_window_moves_only_when_its_opener_changes() {
        let r = |x: f32, y: f32, w: f32, h: f32| egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(w, h));
        let main = r(100.0, 100.0, 1000.0, 600.0);
        assert!(opener_changed(None, main), "the first frame places it");
        assert!(!opener_changed(Some(main), main), "the opener has not moved: nothing to do");
        assert!(!opener_changed(Some(main), r(100.3, 100.0, 1000.0, 600.0)), "sub-point drift is the same place");
        assert!(opener_changed(Some(main), r(150.0, 100.0, 1000.0, 600.0)), "the opener moved");
        assert!(opener_changed(Some(main), r(100.0, 100.0, 1200.0, 720.0)), "the opener was resized");
    }
}

/// 051 — one spawned child window: its form body plus the window dressing
/// the viewport is re-declared with every frame.
pub(crate) struct ChildWindow {
    pub(crate) handle: String,
    pub(crate) body: FormBody,
    pub(crate) viewport_id: egui::ViewportId,
    pub(crate) title: String,
    pub(crate) size: egui::Vec2,
    pub(crate) pos: Option<egui::Pos2>,
    pub(crate) decorations: bool,
    /// The form's designed window chrome — ignored until the property audit
    /// (2026-09-26): a child window had title, size and decorations only.
    pub(crate) can_minimize: bool,
    pub(crate) can_maximize: bool,
    pub(crate) full_screen: bool,
    /// The form's `Resizable`: false and the borders do not drag.
    pub(crate) resizable: bool,
    /// The form's `DockToOpener` and `DockGap`: where this window sits
    /// against the window of the form that opened it.
    pub(crate) dock: cobolt_forms::model::DockEdge,
    pub(crate) dock_gap: f32,
    /// The form's `DockLength`: a share of the opener's edge (0 = designed).
    pub(crate) dock_length: u32,
    /// The opener's window rect when this docked window was last placed: it
    /// is placed again only when that changes.
    pub(crate) dock_opener_seen: Option<egui::Rect>,
    /// Whether the operating system now moves this docked window with its
    /// opener (a native child window, macOS).
    pub(crate) dock_attached: bool,
    /// Whether this docked window had the keyboard last frame — its gaining it
    /// brings the group forward.
    pub(crate) dock_focused: bool,
    /// A screen-relative designed `StartPosition`, applied on the first frame
    /// the monitor's size is known, when the caller gave no position.
    pub(crate) pending_start: Option<cobolt_forms::model::FormStartPosition>,
    /// The caller's `formWindowState` override, or the design's — applied as
    /// commands on the first frame (Maximized/Minimized/Fullscreen).
    pub(crate) initial_state: Option<String>,
    pub(crate) init_sent: bool,
    /// A finished interpreter is reported to the supervisor exactly once.
    pub(crate) finish_reported: bool,
    /// Spec 085 — a window whose form carries a SideMenu runs as a shell of
    /// its own: this is its ContentPane. `None` for a plain child window.
    pub(crate) pane: Option<Pane>,
    /// …and its navigation: the rail, the breadcrumb, the chain. Taken out
    /// while the host acts on it after a frame, so it is an `Option`.
    pub(crate) nav: Option<Box<crate::shell::ChildNav>>,
}

/// 051 — a ContentPane occupant: a full form instance shown in the shell's
/// pane instead of the root form. Resident while registered (049 R20) —
/// parked occupants keep their interpreter and storage warm.
/// Spec 056 — what laying out a responsive form needs from its design, beyond
/// its controls and designed size (both already on the body).
#[derive(Clone, Debug)]
pub(crate) struct ResponsiveSpec {
    pub(crate) layout: std::collections::BTreeMap<String, cobolt_forms::PropValue>,
    pub(crate) breakpoints: Vec<cobolt_forms::layout::breakpoints::Breakpoint>,
    /// The operating system's text-size factor (R68), read once per process.
    pub(crate) system_text_factor: f32,
    /// `me::Breakpoint` / `me::FontScale` as the program pinned them (R84).
    pub(crate) pinned_breakpoint: Option<String>,
    pub(crate) pinned_font_scale: Option<f32>,
    /// The window limits, searched again only when the layout changes.
    pub(crate) limits: cobolt_forms::layout::LimitsCache,
    /// The form's `Responsive` switch as designed or as the program last set
    /// it. The form is laid out while it is on OR an obsolete scaling style is
    /// set (spec 081 R4) — see [`Self::lays_out`].
    pub(crate) switched_on: bool,
}

/// Spec 081 — an `ObsoleteScalingStyle` value a program may write: a whole
/// number from 0 to 7.
fn valid_scaling_style(value: &str) -> bool {
    value
        .trim()
        .parse::<i64>()
        .is_ok_and(|v| (0..=cobolt_forms::layout::defaults::SCALING_STYLE_MAX).contains(&v))
}

impl ResponsiveSpec {
    /// `Some` for a form the engine lays out: `Responsive` on, or an obsolete
    /// scaling style set (spec 081 R4).
    pub(crate) fn of(form: &cobolt_forms::Form) -> Option<Self> {
        form.lays_out().then(|| Self::design(form))
    }

    /// The form's responsive design, whether or not it is switched on — what
    /// `me::Responsive = 1` switches to (R84).
    pub(crate) fn design(form: &cobolt_forms::Form) -> Self {
        ResponsiveSpec {
            layout: form.layout.clone(),
            breakpoints: form.breakpoints.clone(),
            system_text_factor: cobolt_forms::text_scale::system_text_factor(),
            pinned_breakpoint: None,
            pinned_font_scale: None,
            limits: Default::default(),
            switched_on: form.responsive,
        }
    }

    /// Whether a form with this design is laid out: `Responsive` on, or an
    /// obsolete scaling style set (spec 081 R4).
    pub(crate) fn lays_out(&self) -> bool {
        self.switched_on
            || cobolt_forms::layout::scale::style(&cobolt_forms::layout::props::FormBag(&self.layout)) != 0
    }

    /// The window sizes these controls lay out for (R18): never below the
    /// form's minimum, and never so small or so large that two controls that
    /// are apart in the design would touch. `(min, max)`.
    pub(crate) fn size_limits(&self, controls: &[cobolt_forms::Control], designed: egui::Vec2) -> (egui::Vec2, egui::Vec2) {
        let l = self.limits.get(controls, (designed.x, designed.y), &self.layout, &self.breakpoints);
        let (mw, mh) = l.window_max();
        (egui::vec2(l.min.0, l.min.1), egui::vec2(mw, mh))
    }

    /// Lay `controls` out for a surface of `available` size; `rail` narrows
    /// a SideMenu the surface draws itself afterwards (R26).
    pub(crate) fn prepare(
        &self,
        ctx: &egui::Context,
        controls: &[cobolt_forms::Control],
        state: &dyn cobolt_forms::render::FormState,
        designed: egui::Vec2,
        available: egui::Vec2,
        rail: Option<(&str, bool)>,
    ) -> cobolt_forms::layout::apply::Prepared {
        let spec = cobolt_forms::layout::apply::FormSpec {
            designed_size: (designed.x, designed.y),
            layout: &self.layout,
            breakpoints: &self.breakpoints,
            system_text_factor: self.system_text_factor,
            pinned_breakpoint: self.pinned_breakpoint.as_deref(),
            pinned_font_scale: self.pinned_font_scale,
        };
        cobolt_forms::layout::apply::prepare_with_rail(ctx, controls, state, &spec, available, rail)
    }
}

pub(crate) struct Occupant {
    pub(crate) handle: String,
    pub(crate) body: FormBody,
    /// What the operator should be told this form IS — its designed Title,
    /// falling back to its form object name when it has none. The breadcrumb
    /// segment reads from here: the chain named loaded forms by their OBJECT
    /// name (`inner-form1`) while the main form used its title, so one strip
    /// showed two vocabularies.
    pub(crate) label: String,
}

/// Spec 085 — one window's ContentPane: the forms loaded into it, which one
/// is showing, and the chrome the shell hands it each frame. The root window
/// owns one (`FormHost::pane`); so does every child window that runs as a
/// shell of its own.
#[derive(Default)]
pub(crate) struct Pane {
    /// 051 R10/R11 — pane occupants, keyed by UPPERCASE form object. Every
    /// entry is resident; `active_occupant` names the one on the pane
    /// (`None` = the window's own form shows).
    pub(crate) occupants: HashMap<String, Occupant>,
    pub(crate) active_occupant: Option<String>,
    /// A COBOL-driven breadcrumb DETAIL level awaiting the shell:
    /// `(form object, text)`, `None` text = cleared.
    pub(crate) pending_crumb_detail: Option<(String, Option<String>)>,
    /// Chrome the SHELL paints over the pane's backdrop and UNDER the form's
    /// controls — its breadcrumb frame. Handed in fresh each frame (the strip
    /// follows the chain, the rail state and the pointer), and painted where
    /// the pane backdrop is: outside the scroll area, so it stays put while
    /// the form scrolls, and before the controls, so a control the developer
    /// placed over the band paints on top of it.
    pub(crate) pane_chrome: Option<Box<dyn Fn(&egui::Painter, egui::Rect)>>,
    /// How tall that chrome band is. The shell form may design controls OVER
    /// the band — it is the shell's own coordinate space. A form LOADED into
    /// the pane may not: it is a different form, and its origin starts below
    /// the band. Only the occupant path reads this.
    pub(crate) pane_band: f32,
    /// Where the last frame actually put the ContentPane's occupant. Recorded
    /// so a test can check an embedded form lands inside the pane instead of
    /// over the MenuPane — the thing that went wrong is a RECT, and nothing
    /// else about the form's state reveals it.
    pub(crate) last_occupant_rect: Option<egui::Rect>,
}

pub struct FormHost {
    /// The ROOT form — the window (or pane occupant) this host started with.
    root: FormBody,
    /// 051 — spawned child windows, one viewport each, re-declared per frame.
    children: Vec<ChildWindow>,
    /// 051 R10/R11 — the root window's ContentPane: its occupants and which
    /// one is showing. A child window that runs as a shell owns a `Pane` of
    /// its own (spec 085).
    pub(crate) pane: Pane,
    /// 051 — the pieces `SpawnWindow` builds a child from (see the config).
    form_req_tx: mpsc::Sender<cobolt_runtime::form_host::FormRequest>,
    form_source: Option<FormSource>,
    child_theme: Option<ChildThemeSource>,
    child_interpreter_setup:
        Option<std::sync::Arc<dyn Fn(&mut cobolt_runtime::interpreter::Interpreter) + Send + Sync>>,
    indexed_engine: cobolt_runtime::indexed::IndexedEngine,
    shared_rust_bridge:
        Option<std::sync::Arc<std::sync::Mutex<cobolt_runtime::rust_bridge::RustBridge>>>,
    /// 049 — own window, or the shell's ContentPane (see [`Surface`]).
    surface: Surface,
    /// 049 — the SideMenu footer Panel and its contents, which the RAIL draws
    /// (`draw_side_menu_footer`) rather than the ContentPane. Empty in a window
    /// host, where the rail is an ordinary control and its footer sits on it
    /// already.
    footer_ids: std::collections::HashSet<String>,
    /// Carries out a toolbar button's platform action, and finishes the window
    /// captures that cannot complete on the frame that asked for them.
    visuals_set: bool,
    quit_sent: bool,
    /// `COBOLT_FRAME_DIAGNOSTICS` — live per-update trace (R27). Without it a
    /// host is a black box: "the label did not change" cannot be told apart
    /// from "the handler never ran" or "the write went to a name that does
    /// not exist".
    diagnostics: bool,
    /// 037 R13 — the form was designed to OPEN minimized; winit has no
    /// pre-minimized builder, so the first frame sends the command once.
    start_minimized: bool,
    /// A screen-relative Start Position (the eight edge/corner positions or
    /// Center) — `None` when the form is `System`/`Custom` (`Custom` is
    /// already in the viewport builder; `System` means "do not touch it").
    /// Read by every [`Self::fit_window_group`] that places the group.
    pending_start_position: Option<cobolt_forms::model::FormStartPosition>,
    /// `ScreenFill`: the percentage and the window's smallest and largest
    /// sizes. Read by every [`Self::fit_window_group`] that places the group,
    /// so it is kept, not consumed.
    pending_screen_fill: Option<(u32, (f32, f32), (f32, f32))>,
    /// The main window's group — it and the windows docked to it — as last
    /// fitted on the screen: the usable area, and each docked window's edge,
    /// gap and thickness. A change fits the group again.
    group_fit_seen: Option<GroupFitKey>,
    /// When the group was first fitted: the docked windows that open within
    /// the next moments are part of the start.
    group_fit_started: Option<std::time::Instant>,
    /// The zoom the windows' limits and sizes were last sent at.
    group_fit_zoom: f32,
    /// 037 — the window lifecycle state machine (vetoes, cascades, handles).
    supervisor: cobolt_runtime::form_host::FormSupervisor,
    /// Requests from the interpreter thread (OpenForm*, handle methods, …).
    form_req_rx: mpsc::Receiver<cobolt_runtime::form_host::FormRequest>,
    /// Broadcasts closed handles back to EVERY interpreter (R24 NULLing).
    closed: ClosedFanout,
    /// Last ACTUAL fullscreen state from ViewportInfo — onFullScreenChanged
    /// fires only on real transitions (R14/AC8). Seeded with the designed
    /// value so opening fullscreen-by-design is not a "change".
    fullscreen_actual: bool,
    /// 056 R18 — the minimum inner size the root window was last given (the
    /// builder's, then every re-send); `None` for a form that is not
    /// responsive, whose window keeps no minimum.
    root_min_inner: Option<(egui::Vec2, egui::Vec2)>,

    // ── 038 window effects ───────────────────────────────────────────────────
    /// Project entrance/exit effects, resolved by the glue
    /// (Default = no effect). The kill-switch env zeroes both at parse time.
    fx_entrance: cobolt_forms::window_fx::FxSpec,
    fx_exit: cobolt_forms::window_fx::FxSpec,
    /// Replay the entrance when the window is restored after minimize (R9).
    fx_restore: bool,
    /// The window was created SEE-THROUGH so its entrance could play over the
    /// desktop (only effects that move/scale/fade the face — see
    /// `WindowEffect::plays_over_desktop`). The form's own `transparency` then
    /// reaches the desktop for the window's whole life, as designed.
    fx_transparent: bool,
    /// The window carries alpha: `fx_transparent`, or a form whose own
    /// `Transparency` is above 0. Decides the viewport's creation, the clear
    /// colour and whether the panel fills; `fx_transparent` alone still
    /// decides how an effect paints.
    pub(crate) see_through: bool,
    /// The title bar is designed to be visible but is currently OFF so the
    /// entrance plays with no fixed chrome; it is switched back on the frame
    /// the animation ends.
    fx_chrome_pending: bool,
    /// The CLIENT rect the entrance played in, held across the frames it takes
    /// the platform to put the chrome back on, so it can be restored.
    ///
    /// Adding a title bar does not mean the same thing everywhere. On macOS the
    /// frame grows outward and the content keeps its size and its place. On
    /// Windows the title bar is carved out of the window rect that already
    /// exists, so the client area shrinks from the top and its origin moves
    /// down — the effect plays across the strip that is about to become the
    /// title bar, and the finished form then appears shifted down by exactly
    /// that height (operator, 2026-09-09, comparing the two platforms).
    ///
    /// Rather than special-case an OS, the rect the effect played in is
    /// recorded and put back. Where the platform already keeps it — macOS —
    /// the recorded and the actual rect agree, and nothing is sent at all.
    fx_chrome_restore: Option<(egui::Rect, u8)>,
    /// One-shot: the chrome was taken off for the EXIT animation.
    fx_chrome_hidden_for_exit: bool,
    /// When the current entrance playback started (first frame, or restore).
    fx_entrance_start: Option<std::time::Instant>,
    /// True once the entrance finished — gates the control load animations
    /// (R8) and hands the frame back to the live UI.
    fx_entrance_done: bool,
    /// When the exit playback started; the actual close fires at its end.
    fx_exit_start: Option<std::time::Instant>,
    /// Deterministic MatrixRain seed (form name + size).
    fx_seed: u32,
    /// Last ACTUAL minimized state from ViewportInfo — the restore replay
    /// triggers on the true→false edge only (R9).
    minimized_actual: bool,
    /// Last ACTUAL maximized state from ViewportInfo, for `onMaximize` /
    /// `onRestore`. Same edge-triggered shape as the two above.
    maximized_actual: bool,
    /// Last observed window focus, for `onGotFocus` / `onLostFocus`.
    focused_actual: bool,
    /// Last observed OS light/dark preference, for `onThemeChanged` and
    /// `onSystemColorChanged` — one signal, and the catalogue offers two names
    /// for it, so both are raised together.
    system_theme_actual: Option<egui::Theme>,
    /// Last observed device pixel ratio, for `onDpiChanged`.
    dpi_actual: Option<f32>,
    /// Last observed window SIZE and POSITION. `onResizing` / `onMoving` fire
    /// while these change; `onResize` / `onMove` fire once when they settle —
    /// the progressive name is the one that repeats. (A CONTROL spells the same
    /// split `onResize` / `onResized`; the form catalogue offers `onResizing`
    /// instead, so the base name is the settled one here.)
    window_size_actual: Option<egui::Vec2>,
    /// The size the last `onResize` reported. A drag that ends where it began
    /// — or a pane that wobbles for a frame while the shell resizes its window
    /// for a rail toggle — settles on no change, and raises nothing.
    window_size_reported: Option<egui::Vec2>,
    /// In a shell, the rail's DESIGNED width: the form's size in its own
    /// coordinates is the ContentPane plus this column (0 in a window).
    rail_dx: f32,
    window_pos_actual: Option<egui::Pos2>,
    /// A resize/move is in flight and its settle event is still owed.
    resize_pending: bool,
    move_pending: bool,
    /// Files were hovering over the window last frame, for the drag trio.
    drag_hovering: bool,
    /// A scroll gesture is in flight, for `onScrollStart` / `onScrollEnd`.
    scrolling: bool,
    /// The pointer was inside the window last frame, for `onMouseEnter` /
    /// `onMouseLeave` (and their pointer aliases).
    pointer_inside: bool,
    /// A pointer button was down last frame, so a pointer that VANISHES can be
    /// told from one merely released — `onPointerCancel`.
    pointer_down: bool,
    /// The per-host seam (R30) — e.g. the compiled application's
    /// `cobolt_windows` replay.
    hooks: Box<dyn HostHooks>,

    // ── 049 Pane-mode observability (the parity suite reads these) ───────────
    /// The rect the pane-fixed backdrop was painted into last frame
    /// (`None` in Window mode, where the engine paints it).
    last_pane_backdrop_rect: Option<egui::Rect>,
    /// The resolved solid fill of that paint — a transparent form leaves the
    /// pane region see-through (R43): alpha 0 here, while the shell chrome
    /// stays opaque.
    last_pane_backdrop_fill: Option<egui::Color32>,
    /// The content scroll offset last frame (the host's own ScrollArea).
    last_content_scroll: egui::Vec2,
    /// 049 R44 — a COBOL-driven MenuPane state change awaiting the shell.
    pending_menu_pane: Option<bool>,
}

impl FormHost {
    /// 049 R42 — every viewport command this host issues funnels through
    /// here: in `Pane` mode the SHELL owns the only window, so a form-issued
    /// window command is a no-op by construction rather than by scattered
    /// guards.
    /// Keep the main window and every window docked to it on the screen
    /// (operator, 2026-10-04: "I was supposed to be able to see all forms no
    /// matter what the resolution"). `ScreenFill` used to size the main window
    /// alone and Start Position to centre it alone, so the bars docked around
    /// a dashboard fell off a smaller screen — and a dashboard whose smallest
    /// layout was larger than the screen had nowhere to go.
    ///
    /// The group is fitted once the monitor and every docked window have
    /// reported, and again whenever the usable screen or the docked set
    /// changes — never every frame, so a window the operator moves or resizes
    /// stays where they put it. `cobolt_forms::model::fit_window_group` does
    /// the geometry: the main window shrinks first, and only a group whose
    /// smallest layout still does not fit zooms the whole application.
    fn fit_window_group(&mut self, ctx: &egui::Context) {
        if self.surface != Surface::Window {
            return;
        }
        let limits = self.root.responsive.as_ref().map(|spec| spec.size_limits(&self.root.controls, self.root.form_size));
        self.fit_group(
            ctx,
            self.pending_screen_fill.map_or(0, |p| p.0),
            self.pending_start_position.unwrap_or(cobolt_forms::model::FormStartPosition::System),
            limits,
        );
    }

    /// [`Self::fit_window_group`] for the window this host's root draws in —
    /// its own, or the application shell's (`shell.rs`, where the form is a
    /// pane and the shell owns `ScreenFill` and Start Position). `limits` are
    /// the window's smallest and largest inner sizes; `None` holds it at the
    /// size it has.
    pub(crate) fn fit_group(
        &mut self,
        ctx: &egui::Context,
        fill: u32,
        start: cobolt_forms::model::FormStartPosition,
        limits: Option<(egui::Vec2, egui::Vec2)>,
    ) {
        use cobolt_forms::model::{DockEdge, DockedExtent};
        let zoom = ctx.zoom_factor();
        let Some((monitor, outer, inner, whole_screen)) = ctx.input(|i| {
            let v = i.viewport();
            let whole = v.maximized.unwrap_or(false) || v.fullscreen.unwrap_or(false);
            Some((v.monitor_size?, v.outer_rect?, v.inner_rect?, whole))
        }) else {
            return;
        };
        if whole_screen {
            return;
        }
        // The windows docked to this one — not those docked to another child.
        let mut docked = Vec::new();
        for c in &self.children {
            if c.dock == DockEdge::None {
                continue;
            }
            let on_root = self
                .supervisor
                .caller_of(&c.handle)
                .is_none_or(|caller| !self.children.iter().any(|w| w.handle == caller));
            if !on_root {
                continue;
            }
            // Not up yet: wait for it rather than fit the group twice.
            if ctx.input_for(c.viewport_id, |i| i.viewport().outer_rect).is_none() {
                return;
            }
            // Its declared size, not the one it reports: right after a zoom
            // the reported one is the old pixels over the new zoom, and would
            // refit the group against a size that is about to change.
            let across = if matches!(c.dock, DockEdge::Top | DockEdge::Bottom) { c.size.y } else { c.size.x };
            docked.push(DockedExtent { edge: c.dock, gap: c.dock_gap, across });
        }
        // The usable screen, in screen points: what the platform reports, or
        // the monitor less room for a task bar.
        let area = cobolt_os_blur::usable_screen_area()
            .map(|(x, y, w, h)| (x as f32, y as f32, w as f32, h as f32))
            .unwrap_or((0.0, 0.0, monitor.x * zoom, (monitor.y * zoom - 48.0).max(1.0)));
        let key: GroupFitKey = (
            [area.0, area.1, area.2, area.3].map(|v| v.round() as i32),
            docked.iter().map(|d| (d.edge, d.gap.round() as i32, d.across.round() as i32)).collect(),
        );
        if self.group_fit_seen.as_ref() == Some(&key) {
            return;
        }
        // A new screen, or the docked windows arriving while the application
        // starts (each opens a frame or more after the last), places the group
        // as at start; a later change only pulls it back onto the screen.
        let started = *self.group_fit_started.get_or_insert_with(std::time::Instant::now);
        let place = self.group_fit_seen.as_ref().is_none_or(|(a, _)| *a != key.0)
            || started.elapsed() < std::time::Duration::from_secs(2);
        self.group_fit_seen = Some(key);
        let size = |r: egui::Rect| (r.width(), r.height());
        let (min, max) = limits.map_or((size(inner), size(inner)), |(a, b)| ((a.x, a.y), (b.x, b.y)));
        let fit = cobolt_forms::model::fit_window_group(
            area,
            ((outer.width() - inner.width()) * zoom, (outer.height() - inner.height()) * zoom),
            fill,
            min,
            max,
            size(inner),
            (outer.min.x * zoom, outer.min.y * zoom),
            &docked,
            start,
            place,
        );
        // A new zoom takes effect on the next frame, and a size sent now would
        // reach the platform worked out at the old one: change the zoom, and
        // fit again once it is in force.
        if (fit.zoom - zoom).abs() > 1e-3 {
            ctx.set_zoom_factor(fit.zoom);
            self.group_fit_seen = None;
            ctx.request_repaint();
            return;
        }
        let zoomed = (self.group_fit_zoom - zoom).abs() > 1e-3;
        if zoomed {
            self.group_fit_zoom = zoom;
            // A window's limits and size reach the platform in pixels, worked
            // out at the zoom they were sent with: send them again, limits
            // first, so the new size is not refused by the old floor.
            if let Some((min, max)) = limits {
                self.root_min_inner = Some((min, max));
                ctx.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(min));
                ctx.send_viewport_cmd(egui::ViewportCommand::MaxInnerSize(max));
            }
            for c in &mut self.children {
                if let Some(spec) = c.body.responsive.as_ref() {
                    let (min, max) = spec.size_limits(&c.body.controls, c.body.form_size);
                    let extra = c.nav.as_ref().map_or(egui::Vec2::ZERO, |n| {
                        egui::vec2(
                            n.shell.menu_pane_width(),
                            if n.shell.full_height { 0.0 } else { n.shell.breadcrumb_height },
                        )
                    });
                    ctx.send_viewport_cmd_to(c.viewport_id, egui::ViewportCommand::MinInnerSize(min + extra));
                    ctx.send_viewport_cmd_to(c.viewport_id, egui::ViewportCommand::MaxInnerSize(max + extra));
                }
                ctx.send_viewport_cmd_to(c.viewport_id, egui::ViewportCommand::InnerSize(c.size));
                // Docked again against the opener's new size and place.
                c.dock_opener_seen = None;
            }
        }
        let want = egui::vec2(fit.inner.0, fit.inner.1);
        if zoomed || (want - inner.size()).length() > 0.5 {
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(want));
        }
        let pos = egui::pos2(fit.pos.0 / fit.zoom, fit.pos.1 / fit.zoom);
        if zoomed || (pos - outer.min).length() > 0.5 {
            ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(pos));
        }
    }

    fn viewport_cmd(&self, ctx: &egui::Context, cmd: egui::ViewportCommand) {
        if self.surface == Surface::Window {
            ctx.send_viewport_cmd(cmd);
        }
    }

    /// 037 — execute the supervisor's decisions against the real window.
    /// Runs a worklist so follow-up actions (e.g. releasing a pending child
    /// spawn) are applied in the same frame.
    fn apply_host_actions(
        &mut self,
        ctx: &egui::Context,
        actions: Vec<cobolt_runtime::form_host::HostAction>,
    ) {
        use cobolt_runtime::form_host::{HostAction, ROOT_HANDLE};
        let mut work = actions;
        while !work.is_empty() {
            let mut next = Vec::new();
            for act in work {
                match act {
                    HostAction::SpawnWindow {
                        handle,
                        form_id,
                        window_state,
                        x,
                        y,
                        width,
                        height,
                        modal: _,
                    } => {
                        // 051 R6 — the real thing. A failed spawn is a VISIBLE
                        // runtime error and the handle is released so the
                        // caller resumes with NULL (R15) — never a silent drop.
                        if let Err(e) =
                            self.spawn_child(&handle, &form_id, window_state, x, y, width, height)
                        {
                            println!("Runtime error: cannot open form '{form_id}': {e}");
                            eprintln!("form-host: OpenForm(\"{form_id}\") failed: {e}");
                            next.extend(self.supervisor.form_finished(&handle));
                        }
                    }
                    HostAction::CloseWindow { handle } => {
                        if handle == ROOT_HANDLE {
                            // 038 R10 — an allowed close plays the exit effect
                            // first; the playback block performs the real
                            // close when the animation completes. Vetoes never
                            // reach this arm, so a refusal plays nothing.
                            if self.fx_exit.is_active() && !self.quit_sent {
                                if self.fx_exit_start.is_none() {
                                    self.fx_exit_start = Some(std::time::Instant::now());
                                }
                            } else {
                                if !self.quit_sent {
                                    self.quit_sent = true;
                                    let _ = self.root.ev_tx.send(FormEvent::quit());
                                }
                                self.viewport_cmd(ctx,egui::ViewportCommand::Close);
                            }
                        } else if let Some(at) =
                            self.children.iter().position(|c| c.handle == handle)
                        {
                            // 051 — a child closes without ceremony: quit its
                            // interpreter (a parked WAIT-EVENT wakes and ends)
                            // and drop the window; the viewport disappears by
                            // not being re-declared next frame.
                            let child = self.children.remove(at);
                            let _ = child.body.ev_tx.send(FormEvent::quit());
                            // Spec 085 — a window that ran as a shell takes the
                            // forms loaded into its pane with it: the
                            // supervisor only detaches embedded forms from a
                            // closing window, so they are released here.
                            if let Some(pane) = child.pane {
                                for occ in pane.occupants.into_values() {
                                    let _ = occ.body.ev_tx.send(FormEvent::quit());
                                    next.extend(self.supervisor.form_finished(&occ.handle));
                                }
                            }
                        } else if let Some((owner, key)) = self
                            .occupant_by_handle(&handle)
                            .map(|(owner, key, _)| (owner.to_string(), key.to_string()))
                        {
                            // 051 — an occupant caught in a close cascade
                            // (application close) goes the same way.
                            if let Some(pane) = self.pane_of_mut(&owner) {
                                if let Some(occ) = pane.occupants.remove(&key) {
                                    let _ = occ.body.ev_tx.send(FormEvent::quit());
                                }
                                if pane.active_occupant.as_deref() == Some(key.as_str()) {
                                    pane.active_occupant = None;
                                }
                            }
                        }
                    }
                    HostAction::FocusWindow { handle } => {
                        if handle == ROOT_HANDLE {
                            self.viewport_cmd(ctx,egui::ViewportCommand::Focus);
                        } else if let Some(vp) = self.child_viewport(&handle) {
                            ctx.send_viewport_cmd_to(vp, egui::ViewportCommand::Focus);
                        }
                    }
                    HostAction::SetWindowState { handle, state } => {
                        let s = state.trim();
                        if handle == ROOT_HANDLE {
                            if s.eq_ignore_ascii_case("Minimized") {
                                self.viewport_cmd(ctx,egui::ViewportCommand::Minimized(true));
                            } else if s.eq_ignore_ascii_case("Maximized") {
                                self.viewport_cmd(ctx,egui::ViewportCommand::Maximized(true));
                            } else {
                                self.viewport_cmd(ctx,egui::ViewportCommand::Minimized(false));
                                self.viewport_cmd(ctx,egui::ViewportCommand::Maximized(false));
                            }
                        } else if let Some(vp) = self.child_viewport(&handle) {
                            if s.eq_ignore_ascii_case("Minimized") {
                                ctx.send_viewport_cmd_to(vp, egui::ViewportCommand::Minimized(true));
                            } else if s.eq_ignore_ascii_case("Maximized") {
                                ctx.send_viewport_cmd_to(vp, egui::ViewportCommand::Maximized(true));
                            } else {
                                ctx.send_viewport_cmd_to(vp, egui::ViewportCommand::Minimized(false));
                                ctx.send_viewport_cmd_to(vp, egui::ViewportCommand::Maximized(false));
                            }
                        }
                    }
                    HostAction::SetFullScreen { handle, on } => {
                        if handle == ROOT_HANDLE {
                            self.viewport_cmd(ctx,egui::ViewportCommand::Fullscreen(on));
                        } else if let Some(vp) = self.child_viewport(&handle) {
                            ctx.send_viewport_cmd_to(vp, egui::ViewportCommand::Fullscreen(on));
                        }
                    }
                    HostAction::SetTitleVisible { handle, on } => {
                        if handle == ROOT_HANDLE {
                            self.root.title_visible = on;
                            self.viewport_cmd(ctx,egui::ViewportCommand::Decorations(on));
                        } else if let Some(vp) = self.child_viewport(&handle) {
                            if let Some(child) = self.children.iter_mut().find(|c| c.handle == handle) {
                                child.body.title_visible = on;
                            }
                            ctx.send_viewport_cmd_to(vp, egui::ViewportCommand::Decorations(on));
                        }
                    }
                    HostAction::NotifyCloseRejected { handle } => {
                        if handle == ROOT_HANDLE {
                            let form = self.root.form_object.clone();
                            self.root.send_event(FormEvent::new(form, "onCloseRejected"));
                        } else if let Some(child) =
                            self.children.iter_mut().find(|c| c.handle == handle)
                        {
                            let form = child.body.form_object.clone();
                            child.body.send_event(FormEvent::new(form, "onCloseRejected"));
                        }
                    }
                    // 049 — a property written THROUGH the supervisor
                    // (`super::X = …` from another form). Forward it to this
                    // form's interpreter (the FullScreen-echo route) so its
                    // own `me::X` reads stay coherent. Visible application
                    // (retitle/resize) lands with the shell host work.
                    HostAction::SetFormProperty { handle, key, value } => {
                        // 049/051 — the write-through's target can be ANY of
                        // the three bodies a handle can name: the root, a
                        // real child window, or a ContentPane OCCUPANT (an
                        // occupant has no window of its own, but it has a
                        // handle and its own interpreter, exactly like the
                        // other two). Missing the occupant case here meant
                        // `super::"SetProperty"` aimed at a form embedded in
                        // the ContentPane was silently dropped — accepted by
                        // the supervisor (published_prop/GetProperty on that
                        // handle saw it), but never folded into THAT form's
                        // own me::X, so its own me::"GetProperty" never saw
                        // what a modal child it opened had just written
                        // (operator report, PowerDemo3's Call Form demo,
                        // 2026-09-18).
                        let target = if handle == ROOT_HANDLE {
                            Some(&self.root)
                        } else if let Some(c) = self.children.iter().find(|c| c.handle == handle) {
                            Some(&c.body)
                        } else {
                            self.occupant_by_handle(&handle).map(|(_, _, o)| &o.body)
                        };
                        if let Some(body) = target {
                            let _ = body.input_tx.send(StateUpdate {
                                ctrl_id: body.form_object.clone(),
                                prop: key.clone(),
                                value: value.clone(),
                                instance_index: 0,
                            });
                        }
                        // …and made visible on that form's window: a write
                        // through `super::` reaches the target's interpreter by
                        // the line above and never passes the drain that
                        // applies the form's own writes (property audit,
                        // 2026-09-26).
                        let pane = self.surface == Surface::Pane;
                        let child_vp = self.child_viewport(&handle);
                        let body = if handle == ROOT_HANDLE {
                            Some((&mut self.root, (!pane).then_some(egui::ViewportId::ROOT)))
                        } else if let Some(c) = self.children.iter_mut().find(|c| c.handle == handle) {
                            Some((&mut c.body, child_vp))
                        } else {
                            self.occupant_body_by_handle_mut(&handle).map(|b| (b, None))
                        };
                        if let Some((body, vp)) = body {
                            let u = StateUpdate::new(body.form_object.clone(), key, value);
                            body.apply_form_window_update(ctx, &u, vp);
                        }
                    }
                    // `super::"<Procedure>"()` — another form asked this one
                    // to run a procedure of its own. It reaches the target's
                    // interpreter as a reserved event, which runs it the next
                    // time it waits; the program's event loop never sees it.
                    HostAction::CallProcedure { handle, name } => {
                        let target = if handle == ROOT_HANDLE {
                            Some(&mut self.root)
                        } else if let Some(c) = self.children.iter_mut().find(|c| c.handle == handle) {
                            Some(&mut c.body)
                        } else {
                            self.occupant_body_by_handle_mut(&handle)
                        };
                        if let Some(body) = target {
                            body.send_event(
                                FormEvent::new(
                                    body.form_object.clone(),
                                    cobolt_runtime::form_host::FormSupervisor::CALL_PROCEDURE_EVENT,
                                )
                                .with_value(name),
                            );
                        }
                    }
                    // 049 R44 — surfaced for the SHELL host, which applies it
                    // to its MenuPane and persists it (R9). A classic window
                    // host takes it nowhere.
                    HostAction::SetMenuPaneCollapsed { collapsed } => {
                        self.pending_menu_pane = Some(collapsed);
                    }
                    // The breadcrumb's detail level, for the SHELL host. It is
                    // recorded against the form that set it, so a crumb set by
                    // a form the operator has since navigated away from cannot
                    // reappear over someone else's name.
                    HostAction::SetBreadcrumbDetail { handle, text } => {
                        // Spec 085 — recorded on the pane of the window the
                        // form lives in: the root's, or a shell child's.
                        let target = if handle == ROOT_HANDLE {
                            Some((ROOT_HANDLE.to_string(), self.root.form_object.clone()))
                        } else if let Some(c) = self.children.iter().find(|c| c.handle == handle && c.pane.is_some()) {
                            Some((c.handle.clone(), c.body.form_object.clone()))
                        } else {
                            self.occupant_by_handle(&handle)
                                .map(|(owner, key, _)| (owner.to_string(), key.to_string()))
                        };
                        if let Some((owner, form_object)) = target {
                            if let Some(pane) = self.pane_of_mut(&owner) {
                                pane.pending_crumb_detail = Some((form_object, text));
                            }
                        }
                    }
                    HostAction::NotifyClosed { handle } => {
                        self.closed.send(&handle);
                    }
                    HostAction::Exit => {
                        if !self.quit_sent {
                            self.quit_sent = true;
                            let _ = self.root.ev_tx.send(FormEvent::quit());
                        }
                        self.viewport_cmd(ctx,egui::ViewportCommand::Close);
                    }
                }
            }
            work = next;
        }
    }

    /// Test-only: register an open with the supervisor so a manually driven
    /// `SpawnWindow` has a live handle to release (the emitted action is
    /// discarded — the test drives the arm itself).
    #[cfg(test)]
    pub(crate) fn supervisor_open_for_test(&mut self, form_id: &str) -> String {
        let (tx, rx) = mpsc::channel();
        let _ = self
            .supervisor
            .handle_request(cobolt_runtime::form_host::FormRequest::OpenForm {
                caller: cobolt_runtime::form_host::ROOT_HANDLE.into(),
                form_id: form_id.into(),
                sync: false,
                window_state: None,
                x: None,
                y: None,
                width: None,
                height: None,
                modal: false,
                reply: tx,
            });
        rx.try_recv().ok().flatten().unwrap_or_default()
    }

    /// Test-only: register a Sync+modal open of `form_id` by the ROOT with the
    /// supervisor, so `root_modal_blocked()` is true with no real child
    /// spawned (the supervisor only needs the handle to exist). For tests in
    /// other modules of this crate, which cannot reach `supervisor` directly.
    #[cfg(test)]
    pub(crate) fn supervisor_open_modal_for_test(&mut self, form_id: &str) {
        let (tx, _rx) = mpsc::channel();
        let _ = self
            .supervisor
            .handle_request(cobolt_runtime::form_host::FormRequest::OpenForm {
                caller: cobolt_runtime::form_host::ROOT_HANDLE.into(),
                form_id: form_id.into(),
                sync: true,
                window_state: None,
                x: None,
                y: None,
                width: None,
                height: None,
                modal: true,
                reply: tx,
            });
    }

    /// The shell's `open-form:` probe (PowerDemo3 nested-sidebar report): does
    /// `form_id`'s design carry a SideMenu control anywhere in its tree? A
    /// SideMenu paints as a rail; embedded in the ContentPane it sits beside
    /// the shell's own rail, so the shell opens such a target as its own
    /// window instead of an occupant. Flattened exactly as
    /// `build_form_instance` flattens for rendering, so this can never
    /// disagree with what actually paints. `&self` — a plain `Fn` lookup,
    /// safe to call again right before `ensure_occupant` resolves the same id.
    /// The id of the SideMenu `form_id` carries, if it carries one.
    pub(crate) fn form_side_menu_id(&self, form_id: &str) -> Result<Option<String>, String> {
        let Some(source) = &self.form_source else {
            return Err("this host has no form source (single-form runtime)".into());
        };
        let (form, _program) = source(form_id)?;
        let mut flat: Vec<cobolt_forms::Control> = Vec::new();
        crate::flatten_controls(&form.controls, &mut flat);
        Ok(flat
            .iter()
            .find(|c| c.control_type == cobolt_forms::ControlType::SideMenu)
            .map(|c| c.id.clone()))
    }

    pub fn form_has_side_menu(&self, form_id: &str) -> Result<bool, String> {
        let Some(source) = &self.form_source else {
            return Err("this host has no form source (single-form runtime)".into());
        };
        let (form, _program) = source(form_id)?;
        let mut flat: Vec<cobolt_forms::Control> = Vec::new();
        crate::flatten_controls(&form.controls, &mut flat);
        Ok(flat
            .iter()
            .any(|c| c.control_type == cobolt_forms::ControlType::SideMenu))
    }

    /// 051 R10 — make sure a pane occupant for `form_id` exists (building
    /// its instance and registering its Embedded handle on first need) and
    /// return its event sender for the shell's `Resident` lifecycle box.
    /// A parked occupant is simply found again — its storage was the point.
    pub fn ensure_occupant(
        &mut self,
        form_id: &str,
    ) -> Result<mpsc::Sender<FormEvent>, String> {
        self.ensure_occupant_in(cobolt_runtime::form_host::ROOT_HANDLE, form_id)
    }

    /// Spec 085 — the ContentPane of the window `owner` (the root handle, or
    /// a child window that runs as a shell). `None` = no such pane.
    pub(crate) fn pane_of(&self, owner: &str) -> Option<&Pane> {
        if owner == cobolt_runtime::form_host::ROOT_HANDLE {
            return Some(&self.pane);
        }
        self.children.iter().find(|c| c.handle == owner).and_then(|c| c.pane.as_ref())
    }

    pub(crate) fn pane_of_mut(&mut self, owner: &str) -> Option<&mut Pane> {
        if owner == cobolt_runtime::form_host::ROOT_HANDLE {
            return Some(&mut self.pane);
        }
        self.children.iter_mut().find(|c| c.handle == owner).and_then(|c| c.pane.as_mut())
    }

    /// Spec 085 — every ContentPane with the window that owns it: the root's,
    /// then each shell child window's.
    fn panes(&self) -> impl Iterator<Item = (&str, &Pane)> {
        std::iter::once((cobolt_runtime::form_host::ROOT_HANDLE, &self.pane)).chain(
            self.children
                .iter()
                .filter_map(|c| c.pane.as_ref().map(|p| (c.handle.as_str(), p))),
        )
    }

    /// The occupant registered under the supervisor `handle`, in whichever
    /// window's pane it lives: `(that window's handle, the occupant's pane key,
    /// the occupant)`.
    fn occupant_by_handle(&self, handle: &str) -> Option<(&str, &str, &Occupant)> {
        self.panes().find_map(|(owner, p)| {
            p.occupants
                .iter()
                .find(|(_, o)| o.handle == handle)
                .map(|(k, o)| (owner, k.as_str(), o))
        })
    }

    fn occupant_body_by_handle_mut(&mut self, handle: &str) -> Option<&mut FormBody> {
        if let Some(o) = self.pane.occupants.values_mut().find(|o| o.handle == handle) {
            return Some(&mut o.body);
        }
        self.children
            .iter_mut()
            .filter_map(|c| c.pane.as_mut())
            .find_map(|p| p.occupants.values_mut().find(|o| o.handle == handle))
            .map(|o| &mut o.body)
    }

    /// The form that owns the window `owner` — the root form, or the child
    /// window's own form.
    fn window_body_mut(&mut self, owner: &str) -> Option<&mut FormBody> {
        if owner == cobolt_runtime::form_host::ROOT_HANDLE {
            return Some(&mut self.root);
        }
        self.children.iter_mut().find(|c| c.handle == owner).map(|c| &mut c.body)
    }

    /// [`Self::ensure_occupant`] for the ContentPane of the window `owner`:
    /// the occupant is opened with that window's form as its caller, so its
    /// `super::` is the form it was loaded into.
    pub fn ensure_occupant_in(
        &mut self,
        owner: &str,
        form_id: &str,
    ) -> Result<mpsc::Sender<FormEvent>, String> {
        let key = form_id.trim().to_ascii_uppercase();
        let Some(pane) = self.pane_of(owner) else {
            return Err(format!("window '{owner}' has no ContentPane"));
        };
        if let Some(occ) = pane.occupants.get(&key) {
            return Ok(occ.body.ev_tx.clone());
        }
        let handle = self.supervisor.open_embedded(owner, &key);
        let (body, form) = match self.build_form_instance(&handle, form_id) {
            Ok(built) => built,
            Err(e) => {
                // The handle must not linger for an instance that never
                // existed (R15's no-silent-drop, embedded flavour).
                let acts = self.supervisor.form_finished(&handle);
                for act in acts {
                    if let cobolt_runtime::form_host::HostAction::NotifyClosed { handle } = act {
                        self.closed.send(&handle);
                    }
                }
                return Err(e);
            }
        };
        let ev_tx = body.ev_tx.clone();
        let label = if form.title.trim().is_empty() {
            form.name.clone()
        } else {
            form.title.trim().to_owned()
        };
        match self.pane_of_mut(owner) {
            Some(pane) => {
                pane.occupants.insert(key, Occupant { handle, body, label });
            }
            None => {
                // The window closed while the form was being built.
                let _ = body.ev_tx.send(FormEvent::quit());
                return Err(format!("window '{owner}' has no ContentPane"));
            }
        }
        Ok(ev_tx)
    }

    /// What the breadcrumb should call a pane occupant: its designed **Title**,
    /// or its form object name when it has none. `None` = no such occupant.
    pub fn occupant_label(&self, form_object: &str) -> Option<String> {
        self.occupant_label_in(cobolt_runtime::form_host::ROOT_HANDLE, form_object)
    }

    pub fn occupant_label_in(&self, owner: &str, form_object: &str) -> Option<String> {
        self.pane_of(owner)?
            .occupants
            .get(&form_object.trim().to_ascii_uppercase())
            .map(|o| o.label.clone())
    }

    /// 051 R10/R11 — put `form_object` (UPPERCASE; `None` = the root form)
    /// on the pane. The ENTERING side of a swap: a form whose lifecycle pair
    /// already fired gets its `onActivate` here (a fresh instance fires
    /// onShow/onActivate through its own warm-up instead). The leaving
    /// side's `onDeactivate`/`onDestroy` is the NavChain's `Resident` job.
    /// 049 — draw the main form's SideMenu footer Panel into the rail's footer
    /// band. The SHELL owns the band (it lays the rail out); the HOST owns the
    /// controls, their live state and their events, so neither has to learn the
    /// other's half.
    pub fn draw_side_menu_footer(
        &mut self,
        ui: &mut egui::Ui,
        band: egui::Rect,
        behind: egui::Color32,
    ) {
        // The rail's footer is part of the same face the shell disables while
        // a modal child lives; its buttons must not queue clicks either.
        let blocked = crate::debug_link::is_paused() || self.root_modal_blocked();
        self.root.draw_side_menu_footer(ui, band, behind, blocked);
    }

    pub fn show_occupant(&mut self, form_object: Option<&str>) {
        self.show_occupant_in(cobolt_runtime::form_host::ROOT_HANDLE, form_object)
    }

    /// [`Self::show_occupant`] for the ContentPane of the window `owner`;
    /// `None` puts that window's own form back on its pane.
    pub fn show_occupant_in(&mut self, owner: &str, form_object: Option<&str>) {
        let key = form_object.map(|f| f.trim().to_ascii_uppercase());
        let Some(pane) = self.pane_of_mut(owner) else {
            return;
        };
        if key == pane.active_occupant {
            return;
        }
        pane.active_occupant = key.clone();
        match key {
            None => {
                if let Some(body) = self.window_body_mut(owner) {
                    if body.lifecycle_sent {
                        let name = body.form_object.clone();
                        body.send_event(FormEvent::new(name, "onActivate"));
                    }
                }
            }
            Some(k) => {
                if let Some(occ) = self.pane_of_mut(owner).and_then(|p| p.occupants.get_mut(&k)) {
                    // Fresh clocks on re-entry: the render engine owns timers
                    // while on-pane.
                    occ.body.parked_timer_clocks.clear();
                    if occ.body.lifecycle_sent {
                        let name = occ.body.form_object.clone();
                        occ.body.send_event(FormEvent::new(name, "onActivate"));
                    }
                }
            }
        }
    }

    /// 051 R11 — drop the occupants the NavChain destroyed (their
    /// `onDestroy` already fired through the `Resident`): quit each
    /// interpreter and release its Embedded handle.
    pub fn retire_occupants(&mut self, gone: &[String]) {
        self.retire_occupants_in(cobolt_runtime::form_host::ROOT_HANDLE, gone)
    }

    /// [`Self::retire_occupants`] for the ContentPane of the window `owner`.
    pub fn retire_occupants_in(&mut self, owner: &str, gone: &[String]) {
        for form_object in gone {
            let key = form_object.trim().to_ascii_uppercase();
            let removed = self.pane_of_mut(owner).and_then(|p| p.occupants.remove(&key));
            if let Some(occ) = removed {
                let _ = occ.body.ev_tx.send(FormEvent::quit());
                let acts = self.supervisor.form_finished(&occ.handle);
                for act in acts {
                    if let cobolt_runtime::form_host::HostAction::NotifyClosed { handle } = act {
                        self.closed.send(&handle);
                    }
                }
            }
            // `active_occupant` is deliberately left pointing at the retired
            // key: the render path falls through to the root safely, and the
            // caller's follow-up `show_occupant` still sees a CHANGE — which
            // is what fires the entering side's onActivate. Clearing it here
            // silently swallowed that activation.
        }
    }

    /// The pane's current occupant (UPPERCASE form object), `None` = root.
    pub fn active_occupant_form(&self) -> Option<&str> {
        self.pane.active_occupant.as_deref()
    }

    /// Where the last rendered frame placed the ContentPane's occupant, or
    /// `None` if no embedded form was on the pane. The pane's origin is the
    /// whole point (see the occupant branch of `ui_impl`).
    pub fn last_occupant_rect(&self) -> Option<egui::Rect> {
        self.pane.last_occupant_rect
    }

    /// Where the last frame put each control of the form that currently owns
    /// the pane — the active occupant, or the root form when none does.
    ///
    /// Screen coordinates, straight from the render engine. "The control is
    /// missing" and "the control was drawn off the visible surface" look the
    /// same to an operator; this is what tells them apart.
    pub fn last_control_rects(&self) -> &HashMap<String, egui::Rect> {
        match self.pane.active_occupant.as_ref().and_then(|k| self.pane.occupants.get(k)) {
            Some(occ) => &occ.body.last_control_rects,
            None => &self.root.last_control_rects,
        }
    }

    /// Test-only: mark the root's lifecycle pair as already fired, the state
    /// every real run reaches after its warm-up.
    #[cfg(test)]
    pub(crate) fn root_lifecycle_sent_for_test(&mut self) {
        self.root.lifecycle_sent = true;
    }

    /// Test-only: publish a form property without running an interpreter —
    /// the state `MOVE 1 TO me::PreventReset` reaches through the supervisor.
    #[cfg(test)]
    pub(crate) fn publish_prop_for_test(&mut self, form_object: &str, key: &str, value: &str) {
        let up = form_object.trim().to_ascii_uppercase();
        let handle = match self.pane.occupants.get(&up) {
            Some(occ) => occ.handle.clone(),
            None => cobolt_runtime::form_host::ROOT_HANDLE.to_string(),
        };
        self.supervisor
            .note_form_props(&handle, vec![(key.to_string(), value.to_string())]);
    }

    /// Test-only observability: the registered occupants' form objects.
    #[cfg(test)]
    pub(crate) fn occupant_forms(&self) -> Vec<String> {
        let mut v: Vec<String> = self.pane.occupants.keys().cloned().collect();
        v.sort();
        v
    }

    /// Test-only observability: an occupant's supervisor handle — a revived
    /// (preserved) occupant keeps the one it was born with.
    #[cfg(test)]
    pub(crate) fn occupant_handle(&self, form_object: &str) -> Option<String> {
        self.pane.occupants
            .get(&form_object.trim().to_ascii_uppercase())
            .map(|o| o.handle.clone())
    }

    /// 051 Q2 — tick every PARKED body's timers (the root while an occupant
    /// shows; every off-pane occupant always), and schedule the wake-up for
    /// the earliest due tick.
    fn tick_parked_bodies(&mut self, ctx: &egui::Context) {
        let active = self.pane.active_occupant.clone();
        let mut next: Option<std::time::Duration> = None;
        let mut fold = |d: Option<std::time::Duration>, next: &mut Option<std::time::Duration>| {
            if let Some(d) = d {
                *next = Some(next.map_or(d, |n: std::time::Duration| n.min(d)));
            }
        };
        if active.is_some() {
            let d = self.root.tick_parked_timers();
            fold(d, &mut next);
        }
        let keys: Vec<String> = self.pane.occupants.keys().cloned().collect();
        for k in keys {
            if Some(k.as_str()) == active.as_deref() {
                continue;
            }
            if let Some(occ) = self.pane.occupants.get_mut(&k) {
                let d = occ.body.tick_parked_timers();
                fold(d, &mut next);
            }
        }
        if let Some(d) = next {
            ctx.request_repaint_after(d);
        }
    }

    /// 051 R19/R28 — is the ROOT window blocked by a live modal child? The
    /// shell disables its chrome (menu pane, breadcrumb) on this too, so the
    /// whole application face waits together.
    ///
    /// An occupant has no window of its own — whatever it opens paints as a
    /// REAL child window, but the occupant's own body is just a region of the
    /// root viewport — so a modal child of the ACTIVE occupant must block the
    /// root exactly as one of the root's own would. Checking only
    /// `modal_children_of(ROOT_HANDLE)` missed this: a form embedded in the
    /// ContentPane that opened a modal child left the shell fully clickable
    /// underneath it — not disabled, not even lowered behind it — while the
    /// occupant's own COBOL flow sat correctly blocked inside `OpenFormSync`
    /// (operator report, PowerDemo3's Call Form demo, 2026-09-18).
    pub fn root_modal_blocked(&self) -> bool {
        if !self
            .supervisor
            .modal_children_of(cobolt_runtime::form_host::ROOT_HANDLE)
            .is_empty()
        {
            return true;
        }
        let Some(key) = &self.pane.active_occupant else {
            return false;
        };
        let Some(occ) = self.pane.occupants.get(key) else {
            return false;
        };
        !self.supervisor.modal_children_of(&occ.handle).is_empty()
    }

    /// 051 R19/R28 — while the root face is blocked (a live modal child, or
    /// the debugger), the overlay style the BLOCKED form designed: the active
    /// ContentPane occupant's, else the root form's own. `None` = not
    /// blocked. The shell paints its rail and breadcrumb from this so the
    /// whole window wears one layer.
    pub fn blocked_overlay_style(&self) -> Option<cobolt_forms::model::ModalOverlayStyle> {
        if !(crate::debug_link::is_paused() || self.root_modal_blocked()) {
            return None;
        }
        let style = self
            .pane.active_occupant
            .as_ref()
            .and_then(|key| self.pane.occupants.get(key))
            .map(|occ| occ.body.modal_overlay_style)
            .unwrap_or(self.root.modal_overlay_style);
        Some(style)
    }

    /// 051 — the child window under `handle`, if any.
    fn child_viewport(&self, handle: &str) -> Option<egui::ViewportId> {
        self.children
            .iter()
            .find(|c| c.handle == handle)
            .map(|c| c.viewport_id)
    }

    /// 051 R19/R28 — if `handle` is CURRENTLY a live modal child, the
    /// viewport its caller renders in (so the reactive-refocus mitigation in
    /// `update_children` knows whose focus to watch). `None` when `handle`
    /// is not modal, its caller already closed, or (defensively) the caller
    /// can't be located among the known bodies.
    fn live_modal_caller_viewport(&self, handle: &str) -> Option<egui::ViewportId> {
        let caller = self.supervisor.caller_of(handle)?;
        let is_live_modal = self
            .supervisor
            .modal_children_of(caller)
            .iter()
            .any(|h| h == handle);
        if !is_live_modal {
            return None;
        }
        if caller == cobolt_runtime::form_host::ROOT_HANDLE {
            Some(egui::ViewportId::ROOT)
        } else if let Some(vp) = self.child_viewport(caller) {
            Some(vp)
        } else if let Some((owner, _, _)) = self.occupant_by_handle(caller) {
            // An occupant has no window of its own — it renders inside the
            // window whose pane holds it, so THAT is the viewport whose focus
            // actually matters: the root's, or a shell child's (spec 085).
            if owner == cobolt_runtime::form_host::ROOT_HANDLE {
                Some(egui::ViewportId::ROOT)
            } else {
                self.child_viewport(owner)
            }
        } else {
            None
        }
    }

    /// 051 R3/R6 — build ONE child form: resolve its design + program through
    /// the glue's `FormSource`, spawn its own interpreter over its own
    /// channel set (fan-out registered, shared bridge injected), and push the
    /// window for the per-frame viewport declaration.
    #[allow(clippy::too_many_arguments)]
    fn spawn_child(
        &mut self,
        handle: &str,
        form_id: &str,
        window_state: Option<String>,
        x: Option<i64>,
        y: Option<i64>,
        width: Option<i64>,
        height: Option<i64>,
    ) -> Result<(), String> {
        // Spec 085 — a form that carries a SideMenu runs as a shell of its
        // own in its window: rail, breadcrumb, ContentPane, chain.
        let side_menu = self.form_side_menu_id(form_id)?;
        let (mut body, form) = self.build_form_instance_as(handle, form_id, side_menu.is_some())?;
        body.owns_window = true;
        // A window carries alpha only if it is created with it: a form
        // designed with a corner radius gets a see-through window, so its
        // corners can be rounded whenever its title bar is off.
        body.see_through_window = form.corner_radius > 0;
        let nav = side_menu.map(|id| {
            let menu = cobolt_forms::paint::registered_menu_for(form_id, &id).map(|d| (*d).clone());
            Box::new(crate::shell::ChildNav::for_form(handle, &form, Some(id), menu, body.ev_tx.clone()))
        });
        let designed = nav
            .as_ref()
            .map(|n| n.window_size(&form))
            .unwrap_or(egui::vec2(form.width as f32, form.height as f32));
        let (fw, fh) = (designed.x, designed.y);
        let size = egui::vec2(
            width.map(|w| w as f32).unwrap_or(fw).max(1.0),
            height.map(|h| h as f32).unwrap_or(fh).max(1.0),
        );
        // The caller's position wins; with none, the form's own designed
        // `StartPosition` — a `Custom` X/Y at once, a screen-relative one on
        // the first frame.
        let pos = match (x, y) {
            (Some(px), Some(py)) => Some(egui::pos2(px as f32, py as f32)),
            _ if form.start_position == cobolt_forms::model::FormStartPosition::Custom => {
                Some(egui::pos2(form.x as f32, form.y as f32))
            }
            _ => None,
        };
        // A docked window goes where its opener is; nothing else places it.
        let docked = form.dock_to_opener != cobolt_forms::model::DockEdge::None;
        let pos = pos.filter(|_| !docked);
        let pending_start = (!docked && pos.is_none() && form.start_position.is_screen_relative())
            .then_some(form.start_position);
        let initial_state = window_state.or_else(|| match form.window_state {
            cobolt_forms::model::WindowState::Maximized => Some("Maximized".into()),
            cobolt_forms::model::WindowState::Minimized => Some("Minimized".into()),
            cobolt_forms::model::WindowState::Normal => None,
        });
        self.children.push(ChildWindow {
            handle: handle.to_string(),
            body,
            viewport_id: egui::ViewportId::from_hash_of(("051-child", handle)),
            title: window_title(&form.title, form.name.clone()),
            size,
            pos,
            decorations: form.title_visible,
            can_minimize: form.can_minimize,
            can_maximize: form.can_maximize,
            full_screen: form.full_screen,
            resizable: form.resizable,
            dock: form.dock_to_opener,
            dock_gap: form.dock_gap as f32,
            dock_length: form.dock_length,
            dock_opener_seen: None,
            dock_attached: false,
            dock_focused: false,
            pending_start,
            initial_state,
            init_sent: false,
            finish_reported: false,
            pane: nav.as_ref().map(|_| Pane::default()),
            nav,
        });
        Ok(())
    }

    /// 051 R3 — ONE form instance: its design resolved through the glue's
    /// `FormSource`, its own interpreter spawned over its own channel set
    /// (fan-out joined, shared bridge adopted), its body ready to render —
    /// as a child window or as a pane occupant, the same build.
    fn build_form_instance(
        &mut self,
        handle: &str,
        form_id: &str,
    ) -> Result<(FormBody, cobolt_forms::Form), String> {
        self.build_form_instance_as(handle, form_id, false)
    }

    /// [`Self::build_form_instance`]; `as_shell` lays the form out as a
    /// shell's own form — its SideMenu taken out as the rail (spec 085).
    fn build_form_instance_as(
        &mut self,
        handle: &str,
        form_id: &str,
        as_shell: bool,
    ) -> Result<(FormBody, cobolt_forms::Form), String> {
        let Some(source) = &self.form_source else {
            return Err("this host has no form source (single-form runtime)".into());
        };
        let (form, program) = source(form_id)?;

        // Flatten + z-sort + seed exactly as the glues do for the root form.
        let mut flat: Vec<cobolt_forms::Control> = Vec::new();
        crate::flatten_controls(&form.controls, &mut flat);
        flat.sort_by_key(|c| c.z_order);
        let mut state: HashMap<String, CtrlState> = HashMap::new();
        for c in &flat {
            state.insert(c.id.clone(), CtrlState::from_control(c));
        }
        // With diagnostics on, say what this EMBEDDED form is before its
        // interpreter starts — the root form has had this since 049 R27 and a
        // pane occupant had nothing at all.
        if crate::diagnostics::frame_diagnostics_enabled() {
            crate::diagnostics::embedded_preamble(handle, &form, &flat);
        }

        let (maps_key, search_key) = crate::seeding::resolve_api_keys();
        let seed = crate::seeding::build_object_seed(
            &form,
            &flat,
            maps_key.as_deref(),
            search_key.as_deref(),
        );

        // The child's own channel set; its closed receiver joins the fan-out.
        let (ev_tx, ev_rx) = mpsc::channel::<FormEvent>();
        let (input_tx, input_rx) = mpsc::channel::<StateUpdate>();
        let (state_tx, state_rx) = mpsc::channel::<StateUpdate>();
        let (display_tx, display_rx) = mpsc::channel::<String>();
        let (closed_tx, closed_rx) = mpsc::channel::<String>();
        self.closed.register(closed_tx);
        let pending = Arc::new(AtomicUsize::new(0));
        let finished = Arc::new(AtomicBool::new(false));

        let form_object = form.name.trim().to_ascii_uppercase();
        // How this form spells its control ids: an event the interpreter queues
        // itself must be dispatched under the same literal the generated
        // EVALUATE compares against.
        let control_ids: Vec<String> = form.controls.iter().map(|c| c.id.clone()).collect();
        // Spec 066 — this form's SideMenu designed rows, from the menus the
        // process registered at start-up, so its interpreter can refuse a
        // program's edit to them. The same hand-over `rcrun run-form` and a
        // built application make for the root form.
        let designed_menus: Vec<(String, cobolt_forms::menu::MenuDefinition)> = form
            .controls
            .iter()
            .filter(|c| c.control_type == cobolt_forms::ControlType::SideMenu)
            .filter_map(|c| {
                cobolt_forms::paint::registered_menu_for(form_id, &c.id).map(|d| (c.id.clone(), (*d).clone()))
            })
            .collect();
        // 049 R28/R29 — the caller of THIS handle, resolved now while `self`
        // is still reachable (the supervisor does not cross into the spawned
        // thread below). `open_form`/`open_embedded` both record it at
        // registration, before this function is ever called, so it is
        // already there on both the child-window and the pane-occupant path.
        let super_handle = self.supervisor.caller_of(handle).map(|h| h.to_string());
        {
            let finished = Arc::clone(&finished);
            let pending = Arc::clone(&pending);
            let form_object = form_object.clone();
            let handle = handle.to_string();
            let req_tx = self.form_req_tx.clone();
            let setup = self.child_interpreter_setup.clone();
            let indexed_engine = self.indexed_engine;
            let bridge = self.shared_rust_bridge.clone();
            let err_tx = display_tx.clone();
            // 061 — when this process is being debugged, the form it is about
            // to open joins the session too: its own command channel, its own
            // breakpoints, its own "only my code" scope, under its own
            // supervisor handle. Registered HERE rather than in the thread so
            // the `Attached` announcement reaches the IDE before anything this
            // child emits. `None` in an ordinary run, where this costs one
            // atomic load.
            //
            // One function, every path: a child window (`spawn_child`), a
            // modal child and a SideMenu pane occupant (`ensure_occupant`) all
            // build through here, so none of them can be forgotten.
            let debug_wiring = crate::debug_link::active_router()
                .map(|router| router.register(&handle, &form_object));
            std::thread::spawn(move || {
                // 051 Q1 — one object bridge per process, adopted from
                // construction: seeding this form's own EXEC RUST objects into
                // the shared bridge directly (rather than building with a
                // private one and swapping it in after) is what keeps their
                // handles valid — see `new_with_channels_and_bridge`'s doc.
                let mut interp =
                    cobolt_runtime::interpreter::Interpreter::new_with_channels_and_bridge(
                        program, ev_rx, state_tx, display_tx, bridge,
                    );
                interp.set_control_ids(control_ids);
                // The parent's engine: a child form creates files in the same
                // format as the form that opened it.
                interp.set_indexed_engine(indexed_engine);
                interp.set_input_channel(input_rx);
                interp.set_event_counter(pending);
                interp.set_form_host(req_tx, &handle, &form_object, closed_rx);
                if let Some(sh) = &super_handle {
                    interp.set_super_form(sh);
                }
                // …**running**, never paused at its own first statement: this
                // form is not the one the developer pressed Debug on. It runs
                // and stops where they put a breakpoint. Starting it paused
                // would halt the application every time any form opened.
                if let Some((cmd_rx, ev_tx, bps, scope)) = debug_wiring {
                    interp.attach_debug_channels_running(cmd_rx, ev_tx, bps);
                    interp.set_debug_user_scope(scope);
                }
                if let Some(setup) = setup {
                    setup(&mut interp);
                }
                interp.seed_objects(seed);
                for (id, def) in &designed_menus {
                    interp.set_designed_menu(id, def);
                }
                match interp.run() {
                    Ok(()) => {}
                    Err(e) if e.is_exit_signal() => {}
                    Err(e) => {
                        eprintln!("Runtime error: {e}");
                        let _ = err_tx.send(format!("Runtime error: {e}"));
                    }
                }
                finished.store(true, Ordering::Relaxed);
            });
        }

        // Spec 085 — a window that runs as a shell lays its own form out the
        // way the main window's shell does: the rail is chrome.
        let (flat, footer_ids, side_dx) = if as_shell {
            pane_layout(flat)
        } else {
            (flat, std::collections::HashSet::new(), 0)
        };

        // The child's theme: the glue resolves it (embedded vs on-disk art);
        // without a resolver it paints procedural Liquid Glass.
        let (theme_pack, surface_theme) = match &self.child_theme {
            Some(resolve) => resolve(&form),
            None => (None, cobolt_forms::surface_theme::liquid_glass()),
        };

        let (fw, fh) = ((form.width as f32 - side_dx as f32).max(1.0), form.height as f32);
        let body = FormBody {
            drawn_reported: false,
            form_name: form.name.clone(),
            title_visible: form.title_visible,
            corner_radius: form.corner_radius,
            see_through_window: false,
                pane_window: None,
            // A child window says so where it opens one; an occupant never is.
            owns_window: false,
            last_window_crumb: None,
            // An occupant is a form INSIDE the pane; the rail belongs to the
            // shell's main form, so an occupant has no footer band of its own
            // and nothing is withheld from its content pass. A window that
            // runs as a shell owns its rail, footer band included.
            footer_ids,
            theme_pack,
            surface_theme,
            glass_style: form.glass_style,
            motion_bound: motion_bindings(&form_object, &form.form_events, &flat),
            controls: flat,
            special_names: form.cobol_structure.special_names.clone(),
            state,
            bg_hex: form.background_color.clone(),
            bg_gradient_enabled: form.background_gradient_enabled,
            bg_gradient_start: form.background_gradient_start_color.clone(),
            bg_gradient_end: form.background_gradient_end_color.clone(),
            bg_gradient_direction: form.background_gradient_direction.clone(),
            transparency: form.transparency.clamp(0, 100) as u8,
            bg_image: form.background_image.clone(),
            bg_mode: form.bg_image_mode,
            use_theme_background: form.use_theme_background,
            modal_overlay_style: form.modal_overlay_style,
            form_size: egui::vec2(fw, fh),
            responsive: ResponsiveSpec::of(&form),
                responsive_off: (!form.lays_out()).then(|| ResponsiveSpec::design(&form)),
            ev_tx,
            input_tx,
            state_rx,
            display_rx,
            pending,
            finished,
            start: std::time::Instant::now(),
            lifecycle_sent: false,
            db_dumped: false,
            form_object,
            anim: cobolt_forms::anim::AnimRuntime::new(fw, fh),
            anim_started: true, // spawned instances skip the entrance-gated load anims
            last_frame: None,
            hovered: std::collections::HashSet::new(),
            parked_timer_clocks: HashMap::new(),
            toolbar_runner: cobolt_forms::toolbar_actions::Runner::default(),
            pending_save_as: Vec::new(),
            pending_os_handoff: Vec::new(),
            os_handoff: OsHandoffChannel::default(),
            action_notice: None,
            last_control_rects: HashMap::new(),
                last_layout: None,
                mirrored: None,
                breakpoint_reported: None,
                font_scale_reported: None,
                snackbars: Default::default(),
                viewer_sessions: Default::default(),
        };
        Ok((body, form))
    }

    /// 051 — the per-frame children pass: report finished interpreters to the
    /// supervisor, then re-declare every child viewport (an immediate
    /// viewport that is not re-declared closes — the IDE's own idiom).
    fn update_children(&mut self, ctx: &egui::Context) {
        // A finished child (STOP RUN / runtime error) releases its handle —
        // exactly once.
        let done: Vec<String> = self
            .children
            .iter_mut()
            .filter(|c| !c.finish_reported && c.body.finished.load(Ordering::Relaxed))
            .map(|c| {
                c.finish_reported = true;
                c.handle.clone()
            })
            .collect();
        for handle in done {
            let acts = self.supervisor.form_finished(&handle);
            self.apply_host_actions(ctx, acts);
        }

        let mut close_requests: Vec<String> = Vec::new();
        for i in 0..self.children.len() {
            // The window blocks while ITS OWN modal child lives (R28) — and
            // while the program is stopped in the debugger, which stops every
            // window of the application, not just the root one.
            let blocked = crate::debug_link::is_paused() || {
                let c = &self.children[i];
                !self.supervisor.modal_children_of(&c.handle).is_empty()
                    // Spec 085 — a modal child of the form on this window's
                    // own ContentPane blocks the whole window, as on the root.
                    || c.pane.as_ref().is_some_and(|p| {
                        p.active_occupant
                            .as_ref()
                            .and_then(|k| p.occupants.get(k))
                            .is_some_and(|o| !self.supervisor.modal_children_of(&o.handle).is_empty())
                    })
            };
            // 051 R19/R28 — the OS-level half of "modal". egui/eframe/winit
            // at this version expose no owner/parent-window relationship
            // (`ViewportBuilder` has no such field, and `egui-winit` never
            // calls winit's `with_parent_window`/`with_owner_window`), so
            // there is no way to make the OS itself keep this child above,
            // or refuse to raise, its caller's window — the disable()+
            // overlay in `child_frame` only blocks input to the CONTENT of
            // a viewport, never the OS chrome of a DIFFERENT one. The best
            // available mitigation: while this child is a live modal, keep
            // it always-on-top, and the instant its caller's viewport
            // reports focus (the operator clicked it, or its title bar), a
            // real OS action succeeded — wrestle focus back immediately.
            // This is reactive, not preventive: there is a brief visible
            // flash, `Focus` has no effect on Wayland, and the caller's own
            // title-bar buttons (close/minimize) remain clickable throughout
            // — only the raise-to-front behavior is mitigated (operator
            // report, PowerDemo3's Call Form demo, 2026-09-18).
            let live_modal_caller_vp = self.live_modal_caller_viewport(&self.children[i].handle);
            // The window a docked child sits against: its opener's — another
            // child window, or the main window (an occupant's opener draws in
            // the main window too).
            let dock_opener_vp = (self.children[i].dock != cobolt_forms::model::DockEdge::None).then(|| {
                self.supervisor
                    .caller_of(&self.children[i].handle)
                    .and_then(|caller| self.children.iter().find(|w| w.handle == caller))
                    .map_or(egui::ViewportId::ROOT, |w| w.viewport_id)
            });
            let child = &mut self.children[i];
            if let Some(opener_vp) = dock_opener_vp {
                dock_child_window(ctx, child, opener_vp);
            }
            let mut builder = egui::ViewportBuilder::default()
                .with_title(child.title.clone())
                .with_inner_size(child.size)
                .with_decorations(child.decorations)
                .with_resizable(child.resizable)
                .with_minimize_button(child.can_minimize)
                .with_maximize_button(child.can_maximize)
                .with_fullscreen(child.full_screen);
            if child.body.see_through_window {
                builder = builder.with_transparent(true).with_has_shadow(false);
            }
            if live_modal_caller_vp.is_some() {
                builder = builder.with_always_on_top();
            }
            // 056 R18 — a responsive child's smallest layout as its floor; a
            // form that is not responsive keeps no minimum, as before.
            if let Some(spec) = child.body.responsive.as_ref() {
                let (min, max) = spec.size_limits(&child.body.controls, child.body.form_size);
                // A window that runs as a shell holds the rail and the
                // breadcrumb beside and above the form.
                let extra = child.nav.as_ref().map_or(egui::Vec2::ZERO, |n| {
                    egui::vec2(
                        n.shell.menu_pane_width(),
                        if n.shell.full_height { 0.0 } else { n.shell.breadcrumb_height },
                    )
                });
                builder = builder.with_min_inner_size(min + extra).with_max_inner_size(max + extra);
            }
            if let Some(p) = child.pos {
                builder = builder.with_position(p);
            }
            let vp = child.viewport_id;
            let handle = child.handle.clone();
            if let Some(caller_vp) = live_modal_caller_vp {
                if ctx.input_for(caller_vp, |i| i.viewport().focused).unwrap_or(false) {
                    ctx.send_viewport_cmd_to(vp, egui::ViewportCommand::Focus);
                }
            }
            let mut close_requested = false;
            let mut nav_work: Option<(Option<usize>, bool, bool)> = None;
            let overlay = blocked.then(|| {
                child
                    .pane
                    .as_ref()
                    .and_then(|p| p.active_occupant.as_ref().and_then(|k| p.occupants.get(k)))
                    .map(|o| o.body.modal_overlay_style)
                    .unwrap_or(child.body.modal_overlay_style)
            });
            ctx.show_viewport_immediate(vp, builder, |vp_ui, _class| {
                if let Some(start) = child.pending_start {
                    let ready = vp_ui.input(|i| {
                        let v = i.viewport();
                        Some((v.monitor_size?, v.outer_rect?.size()))
                    });
                    if let Some((monitor, window)) = ready {
                        if let Some((x, y)) = cobolt_forms::model::resolved_start_position(
                            start,
                            (monitor.x, monitor.y),
                            (window.x, window.y),
                        ) {
                            vp_ui.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(x, y)));
                        }
                        child.pending_start = None;
                    }
                }
                if !child.init_sent {
                    child.init_sent = true;
                    if let Some(s) = &child.initial_state {
                        if s.eq_ignore_ascii_case("Maximized") {
                            vp_ui.send_viewport_cmd(egui::ViewportCommand::Maximized(true));
                        } else if s.eq_ignore_ascii_case("Minimized") {
                            vp_ui.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                        } else if s.eq_ignore_ascii_case("FullScreen")
                            || s.eq_ignore_ascii_case("Full Screen")
                        {
                            vp_ui.send_viewport_cmd(egui::ViewportCommand::Fullscreen(true));
                        }
                    }
                }
                // Ctrl+W / Cmd+W closes THIS window, as its close button does.
                if vp_ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::W)) {
                    vp_ui.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                if vp_ui.input(|i| i.viewport().close_requested()) {
                    // The supervisor decides (vetoes, cascades) — cancel the
                    // OS close and route it like every other close.
                    vp_ui.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                    close_requested = true;
                }
                // 049/051 — a child window whose form carries a SideMenu draws
                // its own breadcrumb strip, so the rail keeps the Open/Collapsed
                // control it has on every other surface. A form with no
                // SideMenu produces no strip and nothing changes for it.
                // Spec 085 — a window whose form carries a SideMenu is a
                // shell of its own: the rail, the breadcrumb, and the form on
                // its ContentPane, exactly as in the main window.
                if let (Some(nav), Some(pane)) = (child.nav.as_mut(), child.pane.as_mut()) {
                    if blocked {
                        vp_ui.disable();
                        vp_ui.set_opacity(1.0);
                    }
                    nav.shell.breadcrumb =
                        nav.chain.segments().into_iter().map(|(_, label)| label).collect();
                    // This window is the child's own, rounded by its form.
                    nav.shell.window_arc = child.body.window_arc(vp_ui.ctx());
                    let mut view = ChildPaneView { body: &mut child.body, pane, blocked, overlay };
                    nav.shell.show_with_host(vp_ui, |_ui| {}, &mut view);
                    nav_work = Some((
                        nav.shell.take_breadcrumb_click(),
                        nav.shell.take_reset_request(),
                        nav.shell.take_toggle_request(),
                    ));
                    return;
                }
                let window = vp_ui.max_rect();
                let label = child.title.clone();
                let chrome = child.body.window_crumb_chrome(vp_ui, window, &label);
                if !blocked {
                    child.body.window_crumb_interact(vp_ui);
                }
                child.body.child_frame(vp_ui, blocked, chrome.as_deref());
            });
            if let Some((crumb, reset, toggle)) = nav_work {
                self.after_child_shell_frame(ctx, &handle, crumb, reset, toggle);
            }
            if close_requested {
                close_requests.push(handle);
            }
        }
        for handle in close_requests {
            let acts = self.supervisor.try_close(&handle);
            self.apply_host_actions(ctx, acts);
        }
    }

    /// Spec 085 — the end of a child shell's frame: its menu activations,
    /// breadcrumb click, reset and fold, performed for THAT window — forms
    /// load into its ContentPane with it as their caller.
    fn after_child_shell_frame(
        &mut self,
        ctx: &egui::Context,
        handle: &str,
        crumb_click: Option<usize>,
        reset_click: bool,
        toggle: bool,
    ) {
        let Some(at) = self.children.iter().position(|c| c.handle == handle) else {
            return;
        };
        let Some(mut nav) = self.children[at].nav.take() else {
            return;
        };
        let ev_tx = self.children[at].body.ev_tx.clone();
        let input_tx = self.children[at].body.input_tx.clone();
        let vp = self.children[at].viewport_id;
        let form_req_tx = self.form_req_tx.clone();
        if toggle {
            nav.shell.collapsed = !nav.shell.collapsed;
            nav.persist_collapsed();
            // The window absorbs the rail's change, so the ContentPane keeps
            // the width its form was designed for — as the main window does.
            if let Some(size) = ctx.input_for(vp, |i| i.viewport().inner_rect.map(|r| r.size())) {
                let width = crate::shell::shell_width_for_pane(
                    size.x,
                    nav.shell.menu_open_width,
                    nav.shell.menu_collapsed_width,
                    !nav.shell.collapsed,
                );
                if (width - size.x).abs() >= 0.5 {
                    ctx.send_viewport_cmd_to(vp, egui::ViewportCommand::InnerSize(egui::vec2(width, size.y)));
                }
            }
        }
        {
            let crate::shell::ChildNav { shell, chain, side_menu_ctrl, .. } = &mut *nav;
            let mut cx = crate::shell::NavCtx {
                shell,
                chain,
                side_menu_ctrl,
                owner: handle,
                ev_tx: &ev_tx,
                input_tx: &input_tx,
                form_req_tx: &form_req_tx,
            };
            cx.after_frame(self, crumb_click, reset_click);
        }
        // The window may have closed while its menu acted.
        if let Some(c) = self.children.iter_mut().find(|c| c.handle == handle) {
            c.nav = Some(nav);
        }
    }

    /// 038 — paint one effect frame: the form's STATIC face (background +
    /// every visible control via the shared `draw_control` pipeline, scaled
    /// into whatever geometry the effect chooses) transformed by progress
    /// `t`. Pixel parity with the designer comes free — same painter.
    ///
    /// `entrance` says which direction is playing: on the way IN the controls
    /// that have their own load animation queued behind this effect are left
    /// out of the face entirely (038 R8 — they arrive under their own power
    /// the moment it ends).
    fn paint_fx_frame(
        &self,
        root_ui: &egui::Ui,
        effect: cobolt_forms::window_fx::WindowEffect,
        duration_ms: u32,
        t: f32,
        entrance: bool,
    ) {
        let rect = root_ui.ctx().content_rect();
        let painter = root_ui
            .painter()
            .clone()
            .with_clip_rect(rect)
            .with_layer_id(egui::LayerId::new(
                egui::Order::Foreground,
                egui::Id::new("window_fx"),
            ));
        let bg = cobolt_forms::render::backdrop_color(&self.root.bg_hex, self.root.transparency);
        let backdrop = self.root.backdrop(root_ui.ctx(), rect.size());
        // 056 — a responsive form's effect reveals the form laid out for this
        // window: the picture the live UI shows the moment the effect ends.
        let laid;
        let controls: &[cobolt_forms::Control] = match &self.root.responsive {
            Some(spec) => {
                laid = spec
                    .prepare(
                        root_ui.ctx(),
                        &self.root.controls,
                        &cobolt_forms::render::DesignedState,
                        self.root.form_size,
                        rect.size(),
                        None,
                    )
                    .controls;
                &laid
            }
            None => &self.root.controls,
        };
        let time = self.root.start.elapsed().as_secs_f64();
        cobolt_forms::window_fx::paint_window_fx(
            &painter,
            rect,
            bg,
            t,
            effect,
            self.fx_seed,
            time,
            self.fx_transparent,
            duration_ms,
            &mut |p, target| Self::paint_face(p, target, rect, controls, &backdrop, entrance),
        );
    }

    /// The static face the effects animate: exactly the picture the live UI
    /// hands back — the full backdrop (colour, gradient, theme art or image)
    /// stretched over the window, and every visible control at its DESIGNED
    /// size — mapped from the untransformed `base` rect into whatever
    /// geometry the effect chose. Scaling the controls against the form size
    /// instead would blow them up on a window bigger than the form and snap
    /// them back the moment the animation ended.
    ///
    /// `hide_load_animated` leaves out the controls whose own load animation
    /// is still waiting on this effect (see
    /// [`cobolt_forms::anim::has_load_animation`]): set for an ENTRANCE, where
    /// showing them would stand them at their finished position only for them
    /// to jump back and fly in again the instant the effect ended. Never set
    /// for an exit — by then those animations have long since played, and
    /// hiding the controls would blank them just as the form leaves.
    fn paint_face(
        painter: &egui::Painter,
        target: egui::Rect,
        base: egui::Rect,
        controls: &[cobolt_forms::Control],
        backdrop: &cobolt_forms::render::Backdrop,
        hide_load_animated: bool,
    ) {
        cobolt_forms::render::paint_backdrop(painter, target, backdrop);
        let sx = target.width() / base.width().max(1.0);
        let sy = target.height() / base.height().max(1.0);
        for c in controls
            .iter()
            .filter(|c| c.visible)
            .filter(|c| !(hide_load_animated && cobolt_forms::anim::has_load_animation(c)))
        {
            let mut scaled = c.clone();
            scaled.rect.x = (c.rect.x as f32 * sx).round() as i32;
            scaled.rect.y = (c.rect.y as f32 * sy).round() as i32;
            scaled.rect.w = ((c.rect.w as f32 * sx).round() as i32).max(1);
            scaled.rect.h = ((c.rect.h as f32 * sy).round() as i32).max(1);
            cobolt_forms::paint::draw_control(painter, target.min, &scaled, false, true, 1.0, 1.0, None);
        }
    }
}

/// Keep the operating system's blur behind this application's windows in
/// step with what the form wants (spec 083 R3): on when its theme is
/// see-through, off otherwise. winit blurs the main window where the platform
/// lets it (macOS, KDE on Wayland); `cobolt_os_blur` reaches every window,
/// child forms included (macOS, Windows), and is repeated each second so a
/// window opened since is caught. One form host per process, one UI thread:
/// the state is the thread's.
pub(crate) fn sync_os_blur(frame: &eframe::Frame, want: bool) {
    use std::cell::Cell;
    use std::time::{Duration, Instant};
    thread_local! {
        static STATE: Cell<(Option<bool>, Option<Instant>)> = const { Cell::new((None, None)) };
    }
    let (last, at) = STATE.with(Cell::get);
    if last != Some(want) {
        if let Some(window) = frame.winit_window() {
            window.set_blur(want);
        }
        cobolt_os_blur::set_all_windows(want);
        STATE.with(|s| s.set((Some(want), Some(Instant::now()))));
    } else if want && at.is_none_or(|t| t.elapsed() >= Duration::from_secs(1)) {
        cobolt_os_blur::set_all_windows(true);
        STATE.with(|s| s.set((Some(true), Some(Instant::now()))));
    }
}

impl FormHost {
    /// Whether this host's window wants the desktop behind it blurred.
    pub(crate) fn wants_os_blur(&self) -> bool {
        self.see_through && self.root.surface_theme.see_through()
    }

    /// Whether the shell's window wants the desktop behind it blurred: its
    /// root form's theme is see-through, OR the form shown in the ContentPane
    /// is. The shell window is always transparent (R43), so a Spatial form
    /// embedded under a non-Spatial shell showed the desktop sharp when only
    /// the root was asked (operator, 2026-10-02).
    pub(crate) fn shell_wants_os_blur(&self) -> bool {
        self.root.surface_theme.see_through()
            || self
                .pane.active_occupant
                .as_ref()
                .and_then(|k| self.pane.occupants.get(k))
                .is_some_and(|occ| occ.body.surface_theme.see_through())
    }
}

impl eframe::App for FormHost {
    /// What the framebuffer is cleared to before anything is painted. On a
    /// see-through window (038 — an entrance that plays over the desktop)
    /// this is fully transparent: the form's own backdrop supplies whatever
    /// opacity it was designed with, and everything it does not paint stays
    /// desktop. Otherwise it is the form's own background colour, so no
    /// stray frame of eframe's default grey can show through an effect.
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        // Every viewport is cleared with this one colour, so a see-through
        // CHILD needs it transparent too while it is open.
        if self.see_through || self.children.iter().any(|c| c.body.see_through_window) {
            egui::Color32::TRANSPARENT.to_normalized_gamma_f32()
        } else {
            cobolt_forms::render::backdrop_color(&self.root.bg_hex, 0).to_normalized_gamma_f32()
        }
    }

    // 051 R19/R28 — no native click-through prevention here, deliberately.
    // `-[NSWindow setIgnoresMouseEvents:]` on a modal-blocked root (tried
    // twice, 1.70.69/1.70.70) makes the whole window invisible to
    // hit-testing: a click on the caller falls through to whatever sits
    // behind it — typically the IDE that launched the form — which the OS
    // then ACTIVATES, so no window of ours is key any more and the modal has
    // nothing to steal focus back from (operator report, 2026-09-19: "when
    // clicked does not return focus to the modal"). Strictly worse than the
    // reactive refocus in `update_children`, which this path relies on.
    fn ui(&mut self, root_ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        sync_os_blur(frame, self.wants_os_blur());
        self.ui_impl(root_ui);
    }
}

impl FormHost {
    /// The form's DESIGNED size — the coordinate space its controls live in.
    /// In `Pane` mode this never follows the pane (049 R11/R35).
    pub fn designed_size(&self) -> egui::Vec2 {
        self.root.form_size
    }

    /// 049 R41 — where the pane-fixed backdrop painted last frame (`None` in
    /// Window mode).
    pub fn pane_backdrop_rect(&self) -> Option<egui::Rect> {
        self.last_pane_backdrop_rect
    }

    /// 049 R40 — the host's own content scroll offset last frame.
    pub fn content_scroll(&self) -> egui::Vec2 {
        self.last_content_scroll
    }

    /// 049 R43 — the resolved solid fill of the pane backdrop last frame.
    pub fn pane_backdrop_fill(&self) -> Option<egui::Color32> {
        self.last_pane_backdrop_fill
    }

    /// `run_form` (spec 084 R31): the form the operator is looking at — the
    /// ContentPane's occupant when one is on the pane, else the root form.
    fn script_body(&mut self) -> &mut FormBody {
        match self.pane.active_occupant.clone() {
            Some(k) if self.pane.occupants.contains_key(&k) => &mut self.pane.occupants.get_mut(&k).unwrap().body,
            _ => &mut self.root,
        }
    }

    fn script_body_ref(&self) -> &FormBody {
        match self.pane.active_occupant.as_ref().and_then(|k| self.pane.occupants.get(k)) {
            Some(occ) => &occ.body,
            None => &self.root,
        }
    }

    /// `run_form`: set a control's property as the operator editing it
    /// would — the host's state and the interpreter's both.
    pub(crate) fn script_set_prop(&mut self, ctrl_id: &str, prop: &str, value: &str) {
        let body = self.script_body();
        let key = body.resolve_ctrl_key(ctrl_id);
        body.state_entry_mut(&key).set(prop, value.to_owned());
        let _ = body.input_tx.send(StateUpdate::new(ctrl_id, prop, value));
        body.wake_for_input(ctrl_id);
    }

    /// `run_form`: raise `event` on a control, as the operator would.
    pub(crate) fn script_event(&mut self, ctrl_id: &str, event: &str) {
        let body = self.script_body();
        body.send_event(FormEvent::new(ctrl_id, event));
        // The program takes an event off the queue BEFORE handling it, so the
        // queue alone says nothing about a handler still running. An input
        // wake behind the event is taken only back in the wait loop — after
        // the handler returns — and is never shown to the program.
        body.wake_for_input(ctrl_id);
    }

    /// Events sent to the form on the pane and not yet handled (with
    /// [`Self::script_event`]: not yet finished).
    pub(crate) fn script_pending(&self) -> usize {
        self.script_body_ref().pending.load(Ordering::Relaxed)
    }

    /// `run_form`: a control's live property on the form on the pane.
    pub(crate) fn script_read(&self, ctrl_id: &str, prop: &str) -> Option<String> {
        let body = self.script_body_ref();
        let key = body.resolve_ctrl_key(ctrl_id);
        body.state.get(&key).and_then(|s| {
            s.props.iter().find(|(k, _)| k.eq_ignore_ascii_case(prop)).map(|(_, v)| v.clone())
        })
    }

    /// The program has ended (STOP RUN, or the form closed).
    pub(crate) fn script_finished(&self) -> bool {
        self.root.finished.load(Ordering::Relaxed)
    }

    /// `run_form` (spec 085): the open child windows — each one's form, and
    /// for a window that runs as a shell, the form on its ContentPane.
    pub(crate) fn script_windows(&self) -> Vec<(String, bool, Option<String>)> {
        self.children
            .iter()
            .map(|c| {
                (
                    c.body.form_object.clone(),
                    c.nav.is_some(),
                    c.pane.as_ref().and_then(|p| p.active_occupant.clone()),
                )
            })
            .collect()
    }

    /// The root form's designed size.
    pub(crate) fn script_form_size(&self) -> egui::Vec2 {
        self.root.form_size
    }

    /// A root-form control's live property, as the program last wrote it
    /// (case-insensitive id and name). Read-only: the shell uses it to draw the
    /// SideMenu's run-time rows and selection (spec 066), which it would
    /// otherwise never see — it draws from its own mounted definition.
    pub fn control_prop(&self, ctrl_id: &str, prop: &str) -> Option<String> {
        let key = self.root.resolve_ctrl_key(ctrl_id);
        self.root.state.get(&key).and_then(|s| {
            s.props
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(prop))
                .map(|(_, v)| v.clone())
        })
    }

    /// 049 R44 — drain a COBOL-driven MenuPane state change (the shell
    /// applies it to `Shell::collapsed` and persists it, R9).
    pub fn take_menu_pane_request(&mut self) -> Option<bool> {
        self.pending_menu_pane.take()
    }

    /// Drain a COBOL-driven breadcrumb detail level: `(form object, text)`,
    /// with `None` text meaning the form cleared it.
    pub fn take_breadcrumb_detail(&mut self) -> Option<(String, Option<String>)> {
        self.pane.pending_crumb_detail.take()
    }

    /// [`Self::take_breadcrumb_detail`] for the window `owner`.
    pub fn take_breadcrumb_detail_in(&mut self, owner: &str) -> Option<(String, Option<String>)> {
        self.pane_of_mut(owner)?.pending_crumb_detail.take()
    }

    /// The rounded window a shell application's main form asks for: the whole
    /// viewport, at the main form's `CornerRadius`, while it has no title bar.
    /// The rail, the breadcrumb strip and the ContentPane are each a piece of
    /// it and round only the corners they reach.
    pub fn shell_window_arc(&self, ctx: &egui::Context) -> Option<cobolt_forms::paint::ContainerClip> {
        if self.root.title_visible {
            return None;
        }
        cobolt_forms::render::window_arc(ctx.content_rect(), self.root.corner_radius)
    }

    /// The chrome the shell paints between the pane's backdrop and the form's
    /// controls. Set fresh each frame; `None` clears it.
    /// `band` is the chrome's own height, which a pane OCCUPANT starts below.
    pub fn set_pane_chrome(
        &mut self,
        chrome: Option<Box<dyn Fn(&egui::Painter, egui::Rect)>>,
        band: f32,
    ) {
        self.pane.pane_chrome = chrome;
        self.pane.pane_band = band;
    }

    /// Read a published form property (what `super::X` reads) off a pane
    /// occupant, or off the ROOT form when `form_object` names it or is
    /// `None`. The shell asks for `PreventReset` before starting a form over.
    pub fn published_form_prop(&self, form_object: Option<&str>, key: &str) -> Option<String> {
        self.published_form_prop_in(cobolt_runtime::form_host::ROOT_HANDLE, form_object, key)
    }

    /// [`Self::published_form_prop`] within the window `owner`: one of its
    /// pane's occupants, or the window's own form.
    pub fn published_form_prop_in(&self, owner: &str, form_object: Option<&str>, key: &str) -> Option<String> {
        let own_object = if owner == cobolt_runtime::form_host::ROOT_HANDLE {
            self.root.form_object.clone()
        } else {
            self.children.iter().find(|c| c.handle == owner)?.body.form_object.clone()
        };
        let handle = match form_object {
            None => owner.to_string(),
            Some(f) => {
                let up = f.trim().to_ascii_uppercase();
                match self.pane_of(owner).and_then(|p| p.occupants.get(&up)) {
                    Some(occ) => occ.handle.clone(),
                    None if up == own_object => owner.to_string(),
                    None => return None,
                }
            }
        };
        self.supervisor.published_prop(&handle, key)
    }

    /// Fire a form-level event at a pane occupant, or at the ROOT form when
    /// `form_object` is `None` or names it. `false` = no such form.
    pub fn notify_form(&mut self, form_object: Option<&str>, event: &str) -> bool {
        self.notify_form_in(cobolt_runtime::form_host::ROOT_HANDLE, form_object, event)
    }

    /// [`Self::notify_form`] within the window `owner`.
    pub fn notify_form_in(&mut self, owner: &str, form_object: Option<&str>, event: &str) -> bool {
        let key = form_object.map(|f| f.trim().to_ascii_uppercase());
        let own = self.window_body_mut(owner).map(|b| b.form_object.clone());
        let body = match &key {
            None => self.window_body_mut(owner),
            Some(k) if Some(k) == own.as_ref() => self.window_body_mut(owner),
            Some(k) => self.pane_of_mut(owner).and_then(|p| p.occupants.get_mut(k)).map(|o| &mut o.body),
        };
        match body {
            Some(b) => {
                let name = b.form_object.clone();
                b.send_event(FormEvent::new(name, event));
                true
            }
            None => false,
        }
    }

    /// 049 R18 (parity observability) — true once no entrance is playing;
    /// a Pane-surface host is born true.
    pub fn entrance_done(&self) -> bool {
        self.fx_entrance_done
    }

    /// 049 — one frame of a `Pane`-surface host, driven by the shell inside
    /// the ContentPane's `Ui` (see [`crate::shell::Shell::show_with_host`]).
    /// The same frame body as a window host; the `Surface` gates neutralise
    /// everything window-only.
    pub fn pane_frame(&mut self, pane_ui: &mut egui::Ui) {
        self.ui_impl(pane_ui);
    }

    /// Publish the MAIN form's theme state — theme pack, glass style and
    /// surface theme — onto `ctx`.
    ///
    /// There is exactly ONE slot for each on the context, and the last writer
    /// wins until the next one. The shell paints the application's own chrome
    /// — the rail, the breadcrumb, the SideMenu's footer Panel — BEFORE it
    /// hands the ContentPane to [`Self::pane_frame`], so with nothing
    /// published on that path every theme-sensitive read in the chrome
    /// answered from whatever had painted last: the OCCUPANT's theme, left
    /// there by `child_frame` at the end of the previous frame. Loading a form
    /// into the pane therefore repainted the main window's own footer in that
    /// form's theme — the operator saw an Elegance form change the drop shadow
    /// on his footer image (2026-09-08: "the footer belongs to the main
    /// window, not to an embedded form").
    ///
    /// Called by both surfaces that paint on the root form's behalf, so the
    /// chrome is the MAIN form's whatever is on the pane.
    pub(crate) fn publish_root_theme(&self, ctx: &egui::Context) {
        cobolt_forms::paint::set_active_theme(ctx, self.root.theme_pack.clone());
        cobolt_forms::paint::set_glass_style(ctx, self.root.glass_style);
        cobolt_forms::paint::set_surface_theme(ctx, self.root.surface_theme.clone());
    }

    /// One frame of the host. Split from [`eframe::App::ui`] (which only adds
    /// the unused `Frame` parameter) so the parity suite can drive frames
    /// through `Context::run_ui` headlessly (spec 042 R29).
    pub(crate) fn ui_impl(&mut self, root_ui: &mut egui::Ui) {
        // Form windows render through Context-level panels; only the Context
        // is needed per frame.
        let ctx = root_ui.ctx().clone();
        let ctx = &ctx;
        // Light visuals baseline — a fresh egui context defaults to DARK mode,
        // which leaks dark widget fills (labels, text boxes) into the form and
        // breaks parity with the designer canvas. Set once.
        if !self.visuals_set {
            self.visuals_set = true;
            ctx.set_visuals(egui::Visuals::light());
        }
        // 038 — put the client area back where the entrance played, if adding
        // the title bar took it out of that rect rather than growing the frame
        // around it (see `fx_chrome_restore`). The platform is not asked which
        // it does: the recorded rect is compared with the real one, so where
        // they already agree this sends nothing.
        if let Some((want, tries)) = self.fx_chrome_restore {
            let now = ctx.input(|i| i.viewport().inner_rect);
            match now {
                // Moved or shrunk: give the client area its size back and pull
                // the frame up by however much of it sits above that area, so
                // the form lands exactly where the effect left it.
                Some(now)
                    if (now.size() - want.size()).length() > 0.5
                        || (now.min - want.min).length() > 0.5 =>
                {
                    let outer = ctx.input(|i| i.viewport().outer_rect);
                    self.viewport_cmd(ctx, egui::ViewportCommand::InnerSize(want.size()));
                    if let Some(outer) = outer {
                        let frame_offset = now.min - outer.min;
                        self.viewport_cmd(
                            ctx,
                            egui::ViewportCommand::OuterPosition(want.min - frame_offset),
                        );
                    }
                    self.fx_chrome_restore = None;
                }
                // Already right — macOS keeps the content in place, so the
                // correction is a no-op there and stops asking.
                Some(_) => self.fx_chrome_restore = None,
                // The window has not reported yet; try a few more frames and
                // then stop rather than watching for ever.
                None => {
                    self.fx_chrome_restore =
                        tries.checked_sub(1).map(|left| (want, left)).filter(|_| tries > 1);
                }
            }
        }

        // 037 R13 — a form designed to open Minimized minimizes on its first
        // frame (one-shot; the builder cannot pre-minimize).
        if self.start_minimized {
            self.start_minimized = false;
            self.viewport_cmd(ctx,egui::ViewportCommand::Minimized(true));
        }
        // ScreenFill, Start Position, and the whole group on the screen: the
        // main window and the windows docked to it are sized and placed
        // together, once the monitor and every docked window have reported,
        // and again whenever the screen changes (`fit_window_group`).
        self.fit_window_group(ctx);
        // Theme pack + glass style for the unified painter (per frame — same
        // contract every host follows).
        self.publish_root_theme(ctx);

        // 047 R6 — Knob/Gauge/Switch/FileDropZone are real widgets from the
        // palette crate; they read their theme from the context and otherwise
        // fall back to an *un-installed* default. Installing it makes the
        // palette explicit rather than accidental, and registers the bundled
        // symbol font so their glyphs render instead of tofu.
        //
        // Deliberately host-only. `install` calls `global_style_mut`, and the
        // IDE drives every form window through `show_viewport_immediate` — one
        // shared Context for the whole application — so doing this there would
        // restyle the IDE's own panels, toolbars and editor around the canvas.
        // This process hosts nothing but the form, so the Context is ours to
        // style. The IDE keeps the crate's documented slate fallback, which is
        // the same palette Elegance uses, so those four widgets match there
        // too (spec 047 plan R-5).
        //
        // Cheap per frame by design: it early-returns when the theme is
        // unchanged. Themes with no such widgets do nothing here (050 — the
        // trait's default is a no-op), so this is no longer a test for one
        // particular theme.
        self.root.surface_theme.install_widget_visuals(ctx);

        // The per-host seam (R30): e.g. the compiled application replays its
        // EXEC RUST block windows here, every frame — that is what
        // `show_viewport_deferred` requires (miss a frame and the window
        // closes). A block cannot do this itself: it runs once, off-thread.
        self.hooks.per_frame(ctx);

        // Program ended (STOP RUN / runtime error) → close the window — via
        // the exit effect when one is configured (038 R10, plan D6: one close
        // choreography regardless of why the window closes).
        if self.root.finished.load(Ordering::Relaxed) {
            if self.fx_exit.is_active() && self.fx_exit_start.is_none() {
                self.fx_exit_start = Some(std::time::Instant::now());
            }
            if self.fx_exit_start.is_none() {
                self.viewport_cmd(ctx,egui::ViewportCommand::Close);
                return;
            }
            // An exit is playing — fall through to its playback block below.
        }
        // Window close button → ONE close path through the supervisor (037
        // R17): a Waiting form vetoes the close (CancelClose + the
        // onCloseRejected event); a Ready form quits as before. The OS close
        // is ALSO cancelled when an exit effect is about to play — the
        // playback block performs the real close when the animation ends
        // (038 R10; the veto fires FIRST, so a refused close plays nothing).
        // Ctrl+W — Cmd+W on macOS — asks the main window to close, exactly as
        // its close button does: the same request, so the same veto, cascade
        // and exit effect (operator, 2026-10-03). A shell window takes its own
        // (`ShellApp::ui`).
        if self.surface == Surface::Window
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::W))
        {
            self.viewport_cmd(ctx, egui::ViewportCommand::Close);
        }
        if ctx.input(|i| i.viewport().close_requested()) && !self.quit_sent {
            let acts = self
                .supervisor
                .try_close(cobolt_runtime::form_host::ROOT_HANDLE);
            let closing = acts.iter().any(|a| {
                matches!(
                    a,
                    cobolt_runtime::form_host::HostAction::CloseWindow { handle }
                        if handle == cobolt_runtime::form_host::ROOT_HANDLE
                )
            });
            if !closing || (self.fx_exit.is_active() && self.fx_exit_start.is_none()) {
                self.viewport_cmd(ctx,egui::ViewportCommand::CancelClose);
            }
            self.apply_host_actions(ctx, acts);
        }

        // 037 — interpreter → supervisor requests (OpenForm*, me:: window
        // methods, handle methods). Drained every frame.
        let mut reqs = Vec::new();
        while let Ok(r) = self.form_req_rx.try_recv() {
            reqs.push(r);
        }
        for req in reqs {
            // A native file dialog a program is waiting on: opened here, by
            // the one drain every host shares, and answered straight back.
            if let cobolt_runtime::form_host::FormRequest::FileDialog {
                kind,
                title,
                filters,
                directory,
                file_name,
                reply,
            } = req
            {
                crate::file_dialog::answer_program(kind, title, filters, directory, file_name, reply);
                continue;
            }
            let acts = self.supervisor.handle_request(req);
            self.apply_host_actions(ctx, acts);
        }

        // 051 — the children pass: finished interpreters release their
        // handles; every child viewport is re-declared for this frame.
        self.update_children(ctx);
        // Each child window published ITS theme on the shared context before
        // painting, so the root's goes back before the root paints — or a
        // caller blocked by a differently themed Sync child wore the child's
        // look until it closed (operator report, Windows, 2026-09-29).
        self.publish_root_theme(ctx);
        self.root.surface_theme.install_widget_visuals(ctx);

        // 038 R10/R11 — exit playback: once armed (allowed close or program
        // end), the window paints only the receding face and performs the
        // REAL close when t reaches 0. onClose still fires exactly once, at
        // the actual close (R13 — the quit event is what dispatches it).
        if let Some(started) = self.fx_exit_start {
            // The chrome steps aside for the exit too, so the form recedes
            // without a title bar hanging behind it (the window is closing —
            // there is nothing to restore afterwards).
            if self.fx_exit.is_active() && !self.fx_chrome_hidden_for_exit {
                self.fx_chrome_hidden_for_exit = true;
                self.viewport_cmd(ctx,egui::ViewportCommand::Decorations(false));
            }
            let exit_ms = fx_duration_ms(&self.fx_exit, root_ui.max_rect().width());
            let dur = exit_ms.max(1) as f64 / 1000.0;
            let t_lin = 1.0 - (started.elapsed().as_secs_f64() / dur).min(1.0);
            if t_lin <= 0.0 {
                if !self.quit_sent {
                    self.quit_sent = true;
                    let _ = self.root.ev_tx.send(FormEvent::quit());
                }
                self.viewport_cmd(ctx,egui::ViewportCommand::Close);
            } else {
                let t = self
                    .fx_exit
                    .effect
                    .progress(self.fx_exit.easing, t_lin as f32);
                self.paint_fx_frame(root_ui, self.fx_exit.effect, exit_ms, t, false);
                ctx.request_repaint();
            }
            return;
        }

        // 049 — the viewport echoes below read THIS host's window state. In
        // Pane mode the viewport is the SHELL's window, so acting on it would
        // fire bogus form events when the shell fullscreens or minimizes.
        if self.surface == Surface::Window {
            // 056 R18 — the window's floor follows the form's minimum: sent
            // again only when it changes (a COBOL write to a layout property),
            // never every frame. A floor only; the window is never resized.
            // Child windows need nothing here: their builder is rebuilt every
            // frame and egui patches a changed minimum itself.
            // The ceiling follows the same way: where growing would make two
            // controls touch, the window stops.
            if let Some(spec) = self.root.responsive.as_ref() {
                let (min, max) = spec.size_limits(&self.root.controls, self.root.form_size);
                if self.root_min_inner != Some((min, max)) {
                    self.root_min_inner = Some((min, max));
                    ctx.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(min));
                    ctx.send_viewport_cmd(egui::ViewportCommand::MaxInnerSize(max));
                }
            }
            // 037 R14 — onFullScreenChanged fires on ACTUAL transitions only,
            // read back from the viewport (the OS may refuse a request). The
            // live value is mirrored onto the form object first so the handler
            // reads the new state.
            let fs = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
            if fs != self.fullscreen_actual {
                self.fullscreen_actual = fs;
                let _ = self.root.input_tx.send(StateUpdate {
                    ctrl_id: self.root.form_object.clone(),
                    prop: "FullScreen".into(),
                    value: if fs { "1".into() } else { "0".into() },
                    instance_index: 0,
                });
                let form = self.root.form_object.clone();
                self.root.send_event(FormEvent::new(form.clone(), "onFullScreenChanged"));
                // …and the DIRECTIONAL pair beside it. `onFullScreenChanged`
                // makes the handler read `me::FullScreen` to learn which way it
                // went; these two say so by which one arrives, which is what
                // the designer has offered all along.
                self.root.send_event(FormEvent::new(
                    form,
                    if fs { "onFullscreen" } else { "onExitFullscreen" },
                ));
            }

            // 038 R9 — restore-after-minimize replays the ENTRANCE visuals
            // only: no form events, no control-animation replay
            // (`anim_started` stays true). Edge-triggered on the observed
            // minimized transition.
            let minimized = ctx.input(|i| i.viewport().minimized.unwrap_or(false));
            if minimized != self.minimized_actual {
                let was = self.minimized_actual;
                self.minimized_actual = minimized;
                if was && !minimized && self.fx_restore && self.fx_entrance.is_active() {
                    self.fx_entrance_done = false;
                    self.fx_entrance_start = None;
                }
                // The window-state events the designer offers, on the same
                // observed edge the effect replay already used. `onHide` is
                // paired with minimize rather than with teardown: the form is
                // still alive and will come back.
                let form = self.root.form_object.clone();
                if minimized {
                    self.root.send_event(FormEvent::new(form.clone(), "onMinimize"));
                    self.root.send_event(FormEvent::new(form, "onHide"));
                } else {
                    self.root.send_event(FormEvent::new(form, "onRestore"));
                }
            }

            // Maximize is the third window state, tracked the same way. A
            // window leaving maximized without being minimized is also a
            // restore — the catalogue has one `onRestore` for both, so both
            // edges raise it.
            let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
            if maximized != self.maximized_actual {
                self.maximized_actual = maximized;
                let form = self.root.form_object.clone();
                self.root.send_event(FormEvent::new(
                    form,
                    if maximized { "onMaximize" } else { "onRestore" },
                ));
            }

            // ── The rest of the form's own events ────────────────────────────
            //
            // All edge- or gesture-triggered off this frame's input, and all
            // raised with the FORM's id. They were designable and never sent
            // (operator, 2026-09-06); a handler on any of them was dead.
            //
            // Window surface only: in Pane mode the viewport belongs to the
            // SHELL, so reading it here would report the shell's focus, DPI and
            // scrolling as though they were this embedded form's.
            // Settled BEFORE `raise` borrows `self.root`: the hit test reads
            // the rects the engine painted last frame, and the closure below
            // holds a mutable borrow for the rest of the block.
            let pointer_pos = ctx.input(|i| i.pointer.interact_pos());
            let over_control = pointer_pos
                .map(|p| self.root.last_control_rects.values().any(|r| r.contains(p)))
                .unwrap_or(false);
            // "On the form" = the pointer is in the window and NOT on a control.
            let on_background = pointer_pos.is_some() && !over_control;

            let form = self.root.form_object.clone();
            let mut raise = |ev: &str| {
                self.root.send_event(FormEvent::new(form.clone(), ev));
            };

            // Focus.
            let focused = ctx.input(|i| i.viewport().focused.unwrap_or(true));
            if focused != self.focused_actual {
                self.focused_actual = focused;
                raise(if focused { "onGotFocus" } else { "onLostFocus" });
            }

            // The OS light/dark preference. One signal, two catalogue names —
            // a theme change IS a system colour change here — so both fire and
            // a form may bind either.
            let theme = ctx.input(|i| i.raw.system_theme);
            if theme != self.system_theme_actual {
                let first = self.system_theme_actual.is_none();
                self.system_theme_actual = theme;
                // Learning the theme on the first frame is not a change.
                if !first {
                    raise("onThemeChanged");
                    raise("onSystemColorChanged");
                }
            }

            // Device pixel ratio — dragging the window to a display with a
            // different scale factor.
            let dpi = ctx.input(|i| i.viewport().native_pixels_per_point);
            if let Some(now) = dpi {
                match self.dpi_actual {
                    Some(was) if (was - now).abs() > 0.001 => {
                        self.dpi_actual = Some(now);
                        raise("onDpiChanged");
                    }
                    None => self.dpi_actual = Some(now),
                    _ => {}
                }
            }

            // Clipboard. egui reports the GESTURE, whoever it was aimed at, so
            // a form learns that a cut/copy/paste happened in it.
            let (cut, copy, paste) = ctx.input(|i| {
                let mut c = (false, false, false);
                for e in &i.events {
                    match e {
                        egui::Event::Cut => c.0 = true,
                        egui::Event::Copy => c.1 = true,
                        egui::Event::Paste(_) => c.2 = true,
                        _ => {}
                    }
                }
                c
            });
            if cut {
                raise("onCut");
            }
            if copy {
                raise("onCopy");
            }
            if paste {
                raise("onPaste");
            }

            // Files dragged over the window, and dropped on it. `onDragOver`
            // repeats while the pointer hovers — the drag is a gesture with a
            // duration, and a handler that wants to track the position needs
            // the frames. It is bounded by the drag itself.
            let (hovering, dropped) =
                ctx.input(|i| (!i.raw.hovered_files.is_empty(), !i.raw.dropped_files.is_empty()));
            if hovering && !self.drag_hovering {
                raise("onDragEnter");
            }
            if hovering {
                raise("onDragOver");
            }
            if !hovering && self.drag_hovering && !dropped {
                raise("onDragLeave");
            }
            self.drag_hovering = hovering;
            if dropped {
                raise("onDrop");
            }

            // Scrolling. `onScroll` and the two axis events repeat with the
            // gesture; the start/end pair brackets it.
            let delta = ctx.input(|i| i.smooth_scroll_delta);
            let moving_scroll = delta.x.abs() > 0.01 || delta.y.abs() > 0.01;
            if moving_scroll {
                if !self.scrolling {
                    self.scrolling = true;
                    raise("onScrollStart");
                }
                raise("onScroll");
                if delta.x.abs() > 0.01 {
                    raise("onHorizontalScroll");
                }
                if delta.y.abs() > 0.01 {
                    raise("onVerticalScroll");
                }
            } else if self.scrolling {
                self.scrolling = false;
                raise("onScrollEnd");
            }

            // ── Mouse, and its pointer aliases ───────────────────────────────
            //
            // A FORM's mouse events are its BACKGROUND (operator ruling,
            // 2026-09-06 — "B"). A click that lands on a control belongs to
            // that control and stops there; only a click on bare form surface
            // raises the form's event. There is no bubbling, because a COBOL
            // handler has no way to mark an event handled and inventing one
            // would be a language change.
            //
            // The hit test uses the rects the engine actually PAINTED last
            // frame, so what counts as "on a control" is what the developer
            // can see. One frame stale, which no click can outrun.
            //
            // TOUCH & POINTER are aliases, not separate events: egui reports
            // touch AS pointer input on every desktop, so a pointer event that
            // fired only for real touch hardware would be dead on every machine
            // this ships to. Binding both names therefore gives two events per
            // gesture — deliberate, and the alternative was leaving seven more
            // events dead.
            // Enter / leave is about the WINDOW, not the background: a pointer
            // crossing onto a control has not left the form, and reporting that
            // as a leave would fire constantly.
            let inside = ctx.input(|i| i.pointer.has_pointer());
            if inside != self.pointer_inside {
                self.pointer_inside = inside;
                if inside {
                    raise("onMouseEnter");
                    raise("onPointerEnter");
                } else {
                    raise("onMouseLeave");
                    raise("onPointerLeave");
                }
            }

            let (pressed, released, clicked, double, context, moving, gesture) =
                ctx.input(|i| {
                    let gesture = i.events.iter().any(|e| {
                        matches!(e, egui::Event::Zoom(_) | egui::Event::Rotate(_))
                    });
                    (
                        i.pointer.any_pressed(),
                        i.pointer.any_released(),
                        i.pointer.primary_clicked(),
                        i.pointer.button_double_clicked(egui::PointerButton::Primary),
                        i.pointer.secondary_clicked(),
                        i.pointer.is_moving(),
                        gesture,
                    )
                });

            if on_background {
                if pressed {
                    raise("onMouseDown");
                    raise("onPointerDown");
                }
                if released {
                    raise("onMouseUp");
                    raise("onPointerUp");
                }
                if clicked {
                    raise("onClick");
                }
                if double {
                    raise("onDoubleClick");
                }
                if context {
                    raise("onContextMenu");
                }
                if moving {
                    raise("onMouseMove");
                    raise("onPointerMove");
                }
            }

            // A press that ends because the pointer VANISHED — dragged out of
            // the window, or the OS took it — is a cancel, not a release.
            if self.pointer_down && !inside {
                raise("onPointerCancel");
            }
            self.pointer_down = if pressed {
                true
            } else if released || !inside {
                false
            } else {
                self.pointer_down
            };

            // The wheel is a WINDOW gesture, so it is not gated on the
            // background — the same reasoning that lets `onScroll` above fire
            // wherever the pointer is.
            if moving_scroll {
                raise("onMouseWheel");
            }
            // Pinch and rotate. These have no mouse counterpart, so `onGesture`
            // is the only name for them.
            if gesture {
                raise("onGesture");
            }
        }

        // Size and position — in Pane mode too. A FormHost in Pane mode is the
        // SHELL's main form (a form loaded into the pane is a body of its own,
        // never this), and the shell's window is that form's window: its size
        // is the form's size. Behind the Window-only guard above, a shell
        // form's onResize was never raised (operator, 2026-09-29: PowerChat's
        // CHAT-FORM onResize breakpoint was never hit).
        {
            let form = self.root.form_object.clone();
            let input_tx = self.root.input_tx.clone();
            let mut raise = |ev: &str| {
                self.root.send_event(FormEvent::new(form.clone(), ev));
            };
            // Size and position. The progressive name repeats while the drag is
            // in flight; the base name fires once when it settles.
            //
            // The new size is mirrored onto the form object first, the way
            // FullScreen is, so the handler reads it: `me::Height` used to stay
            // the designed height, and a handler laying the form out from it
            // computed a change of zero (operator, 2026-09-28).
            let mirror_size = |size: egui::Vec2| {
                for (prop, v) in [("Width", size.x), ("Height", size.y)] {
                    let _ = input_tx.send(StateUpdate {
                        ctrl_id: form.clone(),
                        prop: prop.into(),
                        value: format!("{}", v.round() as i64),
                        instance_index: 0,
                    });
                }
            };
            let (inner, outer) =
                ctx.input(|i| (i.viewport().inner_rect, i.viewport().outer_rect));
            // In a shell the form's size is its ContentPane plus the rail's
            // designed column — the coordinates the form was designed in, and
            // the ones its controls' X and Width are in. NOT the window: a
            // rail toggle resizes the window by the rail's width precisely so
            // the pane keeps its own, and reporting the window made every
            // collapse an `onResize` that shrank the controls by the rail's
            // width (operator, 2026-09-29).
            let form_size = if self.surface == Surface::Pane {
                let pane = root_ui.available_rect_before_wrap().size();
                Some(egui::vec2(pane.x + self.rail_dx, pane.y))
            } else {
                inner.map(|r| r.size())
            };
            if let Some(size) = form_size {
                match self.window_size_actual {
                    Some(was) if (was - size).length() > 0.5 => {
                        self.window_size_actual = Some(size);
                        self.resize_pending = true;
                        mirror_size(size);
                        raise("onResizing");
                    }
                    Some(_) if self.resize_pending => {
                        self.resize_pending = false;
                        let moved = self
                            .window_size_reported
                            .is_none_or(|r| (r - size).length() > 0.5);
                        if moved {
                            self.window_size_reported = Some(size);
                            mirror_size(size);
                            raise("onResize");
                        }
                    }
                    None => {
                        self.window_size_actual = Some(size);
                        self.window_size_reported = Some(size);
                    }
                    _ => {}
                }
            }
            if let Some(rect) = outer {
                let pos = rect.min;
                match self.window_pos_actual {
                    Some(was) if (was - pos).length() > 0.5 => {
                        self.window_pos_actual = Some(pos);
                        self.move_pending = true;
                        raise("onMoving");
                    }
                    Some(_) if self.move_pending => {
                        self.move_pending = false;
                        raise("onMove");
                    }
                    None => self.window_pos_actual = Some(pos),
                    _ => {}
                }
            }
        }

        // ── Animation clock ──────────────────────────────────────────────────
        // Load-time animations start once the WINDOW has fully materialised:
        // the entrance effect completes first, then the controls come alive
        // (038 R8). Without an entrance the gate opens on the first frame,
        // exactly as before.
        if !self.root.anim_started && self.fx_entrance_done {
            self.root.anim_started = true;
            self.root.anim.start_form_load(&self.root.controls);
        }
        let now = std::time::Instant::now();
        let dt = self
            .root
            .last_frame
            .map(|t| now.duration_since(t).as_secs_f32())
            .unwrap_or(0.0);
        self.root.last_frame = Some(now);
        let animating = self.root.anim.tick(dt);

        // Apply property changes coming from the COBOL interpreter. Route each
        // update to the designer-case state key (COBOL upper-cases ids), and
        // repeating-group member writes to the drawn card-instance id.
        let mut drained = 0usize;
        while let Ok(u) = self.root.state_rx.try_recv() {
            // 037 R16 — mirror the form object's FormState into the
            // supervisor so close vetoes see the live value.
            if u.prop.eq_ignore_ascii_case("FormState")
                && u.ctrl_id.eq_ignore_ascii_case(&self.root.form_object)
            {
                self.supervisor.note_form_state(
                    cobolt_runtime::form_host::ROOT_HANDLE,
                    u.value.trim().eq_ignore_ascii_case("Waiting"),
                );
            }
            // COBOL's PLAY ANIMATION / STOP-ANIMATION / PAUSE arrive as writes to
            // these pseudo-properties; act on the write, don't store it.
            //
            // Two entirely different things answer to the same three verbs,
            // by control type. On an ordinary control they trigger a NAMED
            // spec-038 entrance/exit effect (`self.root.anim`, `ctrl.animations`)
            // — unchanged below. On an **Animator** they must instead drive
            // its own GIF/WebP/APNG clock (`cobolt_media`), which
            // `self.root.anim` has no idea exists: routing an Animator here
            // silently touched a HashMap the media player never reads, so
            // Play appeared to work only because AutoPlay was already true,
            // and Pause/Stop did nothing at all — the animation just kept
            // advancing off the wall clock forever (operator, 2026-09-03).
            if let Some(cmd) = anim_command(&u.prop) {
                let is_animator = self
                    .root
                    .controls
                    .iter()
                    .find(|c| c.id.eq_ignore_ascii_case(&u.ctrl_id))
                    .is_some_and(|c| c.control_type == cobolt_forms::ControlType::Animator);
                if is_animator {
                    let media_cmd = match cmd {
                        AnimCommand::Play => cobolt_media::PlaybackCommand::Play,
                        AnimCommand::Stop => cobolt_media::PlaybackCommand::Stop,
                        AnimCommand::Pause => cobolt_media::PlaybackCommand::Pause,
                    };
                    cobolt_media::command(ctx, &u.ctrl_id, media_cmd);
                } else {
                    match cmd {
                        AnimCommand::Play => {
                            self.root.anim
                                .play_programmatic(&self.root.controls, &u.ctrl_id, &u.value)
                        }
                        AnimCommand::Stop => self.root.anim.stop_all(&u.ctrl_id),
                        AnimCommand::Pause => self.root.anim.pause_all(&u.ctrl_id),
                    }
                }
                drained += 1;
                continue;
            }
            // The form's own window properties, on the root's own window —
            // the shell owns it when this host is its pane.
            let own_window = (self.surface != Surface::Pane).then_some(egui::ViewportId::ROOT);
            self.root.apply_form_window_update(ctx, &u, own_window);
            // Routing, the toolbar-button door and the OBSERVER events all live in
            // one place, shared with the child-window path — see
            // `FormBody::apply_interpreter_update` for why that matters.
            self.root.apply_interpreter_update(u, self.diagnostics);
            drained += 1;
        }

        // 058 R18.1/R19/R20 — a Viewer's Save As / SaveAsPdf panel and its
        // Print / Share hand-offs, on the ROOT form too. Only `child_frame`
        // did this, so on a main form — PowerChat's chat, and the first form
        // of every built application — the request was queued and never
        // opened: "Save as PDF" did nothing at all (operator, 2026-09-28).
        self.root.start_pending_save_as(ctx);
        self.root.collect_viewer_save_as();
        self.root.drive_viewer_os_handoffs(ctx);

        // ── One-shot databind diagnostic (opt-in) ────────────────────────────
        // COBOLT_DATABIND_TRACE=1 (also true/on) writes, once, the mismatch
        // between the state keys the interpreter populated and the instanced ids
        // the renderer will look up for each repeating-group member. Decisive for
        // "cards show designed defaults in run-form but not in preview". The IDE
        // sets this from the project's Data-bind trace setting — always (incl.
        // "0"), so test the value rather than mere presence.
        if !self.root.db_dumped
            && crate::diagnostics::databind_trace_enabled()
            && self.root.state.keys().any(|k| k.contains('.'))
        {
            self.root.db_dumped = true;
            self.root.dump_databind_trace();
        }

        // DISPLAY output → stdout (the IDE pipes this into its Output pane).
        // Explicit flush: stdout is BLOCK-buffered when piped, so without it
        // DISPLAY lines sit in the buffer instead of reaching the reader live.
        {
            let mut any = false;
            while let Ok(line) = self.root.display_rx.try_recv() {
                println!("{line}");
                any = true;
            }
            if any {
                use std::io::Write;
                let _ = std::io::stdout().flush();
            }
        }

        // Ignore input for a brief warm-up after the window appears.
        let armed = self.root.start.elapsed().as_millis() > 450;

        // Form-level lifecycle: onShow / onActivate fire once after warm-up
        // (unknown events are ignored by the generated dispatch loop, so this
        // is safe for any form).
        if armed && !self.root.lifecycle_sent {
            self.root.lifecycle_sent = true;
            let name = self.root.form_name.clone();
            self.root.send_event(FormEvent::new(&name, "onShow"));
            self.root.send_event(FormEvent::new(&name, "onActivate"));
        }

        let bg_fill = cobolt_forms::render::backdrop_color(&self.root.bg_hex, self.root.transparency);
        let form_size = self.root.form_size;

        // 038 R7 — entrance playback: until the effect completes, paint the
        // animated STATIC face instead of the live UI (plan D1). Everything
        // interpreter-side above keeps flowing (state drains, onLoad — R13);
        // only the widgets wait. The form is interactive the moment the
        // effect ends.
        if !self.fx_entrance_done {
            let started = *self
                .fx_entrance_start
                .get_or_insert_with(std::time::Instant::now);
            // MatrixRain sets its own floor: one line per 25–50 ms beat is a
            // schedule of its own length, and the configured duration is the
            // MINIMUM it may take, never the maximum (operator, 2026-07-31).
            let ent_ms = fx_duration_ms(&self.fx_entrance, root_ui.max_rect().width());
            let dur = ent_ms.max(1) as f64 / 1000.0;
            let t_lin = (started.elapsed().as_secs_f64() / dur).min(1.0);
            if t_lin >= 1.0 {
                self.fx_entrance_done = true; // live UI takes over this frame
                // …and the load animations start on THIS frame, not the next
                // one. The gate above runs earlier in the pass, so waiting for
                // it would let the live UI paint one frame of every control
                // standing at its finished position — a single-frame flash of
                // exactly the picture the entrance just took care to withhold.
                if !self.root.anim_started {
                    self.root.anim_started = true;
                    self.root.anim.start_form_load(&self.root.controls);
                }
                // The window wears its chrome again, arriving together with
                // the finished form (038 — the title bar was off so nothing
                // stood still while the effect played).
                if self.fx_chrome_pending {
                    self.fx_chrome_pending = false;
                    // Where the effect just played, so it can be put back if
                    // the platform takes the title bar out of it. A maximized
                    // or fullscreen window owns no rect of its own to restore.
                    let settled = ctx.input(|i| {
                        i.viewport().maximized.unwrap_or(false)
                            || i.viewport().fullscreen.unwrap_or(false)
                    });
                    self.fx_chrome_restore = if settled {
                        None
                    } else {
                        ctx.input(|i| i.viewport().inner_rect).map(|r| (r, 8))
                    };
                    self.viewport_cmd(ctx,egui::ViewportCommand::Decorations(true));
                }
            } else {
                let t = self
                    .fx_entrance
                    .effect
                    .progress(self.fx_entrance.easing, t_lin as f32);
                self.paint_fx_frame(root_ui, self.fx_entrance.effect, ent_ms, t, true);
                ctx.request_repaint();
                return;
            }
        }

        // Render the whole form through the unified engine (one renderer for
        // the designer, preview, and every host — spec 017).
        let surface = self.surface;
        // 051 R19/R28 — while a MODAL child of this window (or of its active
        // occupant — `root_modal_blocked` covers both) is open, the whole
        // root face is disabled: it stays visible but takes no input.
        // …and so is a form whose program is STOPPED in the debugger. The
        // interpreter is blocked inside `debug_check` and cannot run a handler,
        // but the GUI thread keeps painting and keeps collecting input — so
        // every click made while reading the code was queued and delivered in a
        // burst on Continue (operator, 2026-09-17). Same treatment as a modal
        // child: visible, legible, taking nothing.
        let root_blocked = crate::debug_link::is_paused() || self.root_modal_blocked();
        // 051 Q2 — parked bodies keep their timers running, whoever owns the
        // pane this frame.
        self.tick_parked_bodies(ctx);
        // A shell's window is rounded by its MAIN form (this root body); the
        // ContentPane is one piece of it, whoever occupies it.
        let shell_arc = (self.surface == Surface::Pane).then(|| self.shell_window_arc(ctx)).flatten();
        if self.surface == Surface::Pane {
            self.root.pane_window = shell_arc;
        }
        // 051 R10 — an active occupant owns the pane; the root form above
        // stayed fully live (its drains ran), just unrendered — parked.
        if let Some(key) = self.pane.active_occupant.clone() {
            let chrome = self.pane.pane_chrome.take();
            let band = self.pane.pane_band;
            if let Some(occ) = self.pane.occupants.get_mut(&key) {
                occ.body.pane_window = shell_arc;
                // 049 — an embedded form is NOT the shell form. The shell may
                // design its own controls over the breadcrumb band, because
                // that band is the shell's own coordinate space; a form LOADED
                // into the pane has its own, and it starts BELOW the band —
                // otherwise its first row of controls lands on the navigation
                // chain, which is what the operator saw.
                //
                // The band is also painted HERE rather than inside the
                // occupant's scroll area, so the chrome does not scroll away
                // with the form's content (the same rule the root path keeps).
                //
                // 051 — and it is the PANE's rect, not the window's. `ui_impl`
                // is handed the shell's ROOT `Ui`, the same surface the
                // MenuPane and (when the breadcrumb is not full-height) the
                // crumb strip were added to as panels; `max_rect()` on it is
                // the whole window and knows nothing about those siblings.
                // The root-form path below never had to think about this
                // because it goes through a `CentralPanel`, which consumes
                // exactly what the panels left — so the shell form landed in
                // the ContentPane while a form LOADED into the pane was drawn
                // from the window's top-left, over the rail, offset by nothing
                // but the band (operator, 2026-08-20). `available_rect_before_wrap`
                // is that same leftover region, and it is what the shell itself
                // records as `ShellLayout::content_rect`. In a plain form
                // WINDOW there are no such siblings, so it is the whole root
                // rect and this path is unchanged.
                let pane_rect = root_ui.available_rect_before_wrap();
                if let Some(chrome) = chrome.as_deref() {
                    chrome(root_ui.painter(), pane_rect);
                }
                let mut rect = pane_rect;
                rect.min.y += band;
                self.pane.last_occupant_rect = Some(rect);
                let mut pane = root_ui.new_child(egui::UiBuilder::new().max_rect(rect));
                occ.body.child_frame(&mut pane, root_blocked, None);
                ctx.request_repaint_after(std::time::Duration::from_millis(200));
                return;
            }
        }
        let mut pane_backdrop_rect: Option<egui::Rect> = None;
        let mut pane_backdrop_fill: Option<egui::Color32> = None;
        let mut content_scroll = egui::Vec2::ZERO;
        // Focus BEFORE this frame's widgets see the click — see child_frame.
        let pre_focus = ctx.memory(|m| m.focused());
        // 055 D3/R16 — this body's surface, taken BEFORE the CentralPanel
        // consumes it. In a plain form window it is the whole viewport; in Pane
        // mode it is what the rail and the breadcrumb left, which is exactly the
        // region the shell records as its content rect. Taken afterwards it
        // would be the already-consumed remainder, and every notification would
        // anchor to the wrong rectangle.
        let snack_surface = root_ui.available_rect_before_wrap();
        // R5/R5.1 — ask this form's Viewer threads for their work and
        // collect what is finished, BEFORE the render borrows the body.
        self.root.tick_viewers(ctx);
        let viewer_docs = self.root.viewer_documents();
        let mut laid_layout: Option<cobolt_forms::layout::LayoutOutput> = None;
        let output = {
            let mut controls = self.root.controls.clone();
            let st = LiveState {
                state: &self.root.state,
                anim: &self.root.anim,
                hidden: Some(&self.root.footer_ids),
                viewer_docs: Some(&viewer_docs),
                special_names: &self.root.special_names,
            };
            let active_tabs = self.root.active_tabs();
            let backdrop = self.root.backdrop(ctx, ctx.content_rect().size());
            let pane_chrome = self.pane.pane_chrome.take();
            let mut out = cobolt_forms::render::RenderOutput::default();
            // On a see-through window the panel must NOT fill: the engine
            // paints the same backdrop across the whole window a moment
            // later, and painting a translucent colour twice would double the
            // form's designed opacity against the desktop. In Pane mode the
            // panel never fills either — the pane-fixed backdrop below is the
            // one and only background paint (049 R41).
            let panel_fill = if self.see_through || surface == Surface::Pane {
                egui::Color32::TRANSPARENT
            } else {
                bg_fill
            };
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE.fill(panel_fill))
                .show(root_ui, |ui| {
                    if root_blocked {
                        ui.disable();
                        // Same as `child_frame`: the form keeps its designed
                        // transparency; only the overlay dims.
                        ui.set_opacity(1.0);
                    }
                    // 049 R12/R13/R41 — Pane mode: the PANE paints the form's
                    // backdrop, sized to the pane, OUTSIDE the scroll area.
                    // The background stays put while the controls scroll
                    // (R41), and gradient/image modes are evaluated against
                    // the PANE rect (R13). The engine then gets a fully
                    // transparent backdrop, so nothing is painted twice.
                    let engine_backdrop = if surface == Surface::Pane {
                        let rect = ui.max_rect();
                        let painted =
                            cobolt_forms::render::paint_backdrop(ui.painter(), rect, &backdrop);
                        pane_backdrop_fill = Some(painted.bg);
                        pane_backdrop_rect = Some(rect);
                        // The shell's breadcrumb frame: on the pane backdrop,
                        // outside the scroll area (chrome does not scroll) and
                        // before the controls, so a control the developer put
                        // over the band paints on top of it. The frame is not
                        // a container — that control is nobody's child.
                        if let Some(chrome) = &pane_chrome {
                            chrome(ui.painter(), rect);
                        }
                        cobolt_forms::render::Backdrop {
                            // `transparency: 100` is what actually makes this
                            // inert. A colour alone cannot: `backdrop_color`
                            // maps pure black to the default navy on purpose —
                            // so that a form with no background set is still a
                            // visible window — and `#00000000` IS pure black to
                            // it. The engine therefore painted OPAQUE NAVY over
                            // the pane backdrop that had just been painted
                            // correctly two lines above, which is why the
                            // ContentPane ignored the background set in the RAD
                            // while the rail and the breadcrumb honoured it.
                            //
                            // `Backdrop::behind` (1.61.156) says this outright
                            // rather than by arithmetic, and is what the footer
                            // band uses. This site is deliberately left on the
                            // transparency trick: it works, and switching it
                            // would also change the backdrop PUBLISHED to the
                            // pane's translucent controls — a visible change
                            // nobody asked for.
                            paint: true,
                            color_hex: "#00000000".into(),
                            transparency: 100,
                            gradient_enabled: false,
                            gradient_start_hex: String::new(),
                            gradient_end_hex: String::new(),
                            gradient_direction: String::new(),
                            image: None,
                            image_mode: cobolt_forms::model::BgImageMode::Stretch,
                            use_theme_background: false,
                            window_size: None,
                            // …but an inert backdrop leaves the engine with no
                            // colour for the corner-notch mask, and it used to
                            // fall back to the ambient `panel_fill` — which this
                            // panel does NOT fill from (Pane fills TRANSPARENT),
                            // and which a self-contained form theme installs
                            // globally and never removes. So the next form's
                            // rounded corners were repainted in the previous
                            // form's palette: black wedges (operator,
                            // 2026-08-23). We painted the pane backdrop two
                            // lines above; tell the engine what it says.
                            behind_fill: Some(painted.bg),
                            image_extent: None,
                            draggable: false,
                            // The pane painted the (rounded) backdrop above;
                            // its top-level controls are still cut to the arc.
                            window: backdrop.window,
                        }
                    } else {
                        backdrop
                    };
                    // Floating scrollbars overlay the content instead of
                    // reserving a gutter, so no light track strip shows on the
                    // right/bottom edges when the form fits (only appears, as an
                    // overlay, if the user shrinks the resizable window).
                    ui.style_mut().spacing.scroll = form_scroll_style();
                    // 056 R23 — the surface a responsive form lays out for:
                    // what this panel offers, taken before the scroll area.
                    let surface_size = ui.available_size();
                    let responsive = &self.root.responsive;
                    let sa = egui::ScrollArea::both()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            let laid_state = cobolt_forms::layout::apply::LaidOutState { inner: &st };
                            let prepared = responsive.as_ref().map(|spec| {
                                spec.prepare(ui.ctx(), &controls, &st, form_size, surface_size, None)
                            });
                            laid_layout = prepared.as_ref().map(|p| p.layout.clone());
                            let (render_controls, render_size, render_state): (
                                &[cobolt_forms::Control],
                                egui::Vec2,
                                &dyn cobolt_forms::render::FormState,
                            ) = match &prepared {
                                // R43 — a responsive form's MenuBar and
                                // StatusBar follow their anchors; nothing
                                // stretches them a second time.
                                Some(p) => (&p.controls, p.form_size, &laid_state),
                                None => {
                                    // A Responsive MenuBar and a StatusBar span
                                    // the window the operator sees, not only
                                    // the width the form was designed at.
                                    cobolt_forms::Form::stretch_window_bars(
                                        &mut controls,
                                        ui.available_width().max(form_size.x),
                                    );
                                    (&controls, form_size, &st)
                                }
                            };
                            ui.set_min_size(render_size);
                            let input = cobolt_forms::render::RenderInput {
                                controls: render_controls,
                                state: render_state,
                                form_size: render_size,
                                glass: true,
                                mode: cobolt_forms::render::RenderMode::Interactive,
                                active_tabs: &active_tabs,
                                backdrop: engine_backdrop,
                            };
                            // Its OWN id space. The shell renders a form here
                            // AND a form fragment in the SideMenu footer, both
                            // through `render_form`, both deriving widget ids
                            // from control ids — so a control in the pane and
                            // one in the rail could hash to the same id and
                            // egui reported "Second use of widget ID …" over a
                            // TextBox that had done nothing wrong (operator,
                            // 2026-09-02). Two surfaces, two id spaces.
                            ui.push_id("content-pane", |ui| {
                                out = cobolt_forms::render::render_form(ui, &input);
                            });
                        });
                    content_scroll = sa.state.offset;
                });
            // 051 R19/R28 — `disable()` above already refuses input; this is
            // only the paint that lets the operator SEE the root is waiting,
            // in the style the ROOT form's own design chose. Mirrors the
            // occupant/child-window overlay in `FormBody::child_frame` — the
            // root's own content render is a separate, hand-inlined copy of
            // that logic rather than a call to it, so the paint has to be
            // added here too.
            if let Some(fill) = if root_blocked {
                modal_overlay_fill(self.root.modal_overlay_style)
            } else {
                None
            } {
                // A fresh painter for the same reason as in `child_frame`: a
                // root form shown in Pane mode sits under the shell's
                // disabled root `Ui`, whose inherited painter would halve
                // the overlay's alpha.
                cobolt_forms::paint::fill_in_clip(
                    &overlay_painter(root_ui),
                    snack_surface,
                    fill,
                    self.root.window_arc(ctx),
                );
            }
            out
        };
        self.last_pane_backdrop_rect = pane_backdrop_rect;
        self.last_pane_backdrop_fill = pane_backdrop_fill;
        self.last_content_scroll = content_scroll;
        // Where the engine actually put every control this frame — see
        // `FormBody::last_control_rects`.
        self.root.last_control_rects = output.control_rects.clone();
        // A main window without a title bar moves by its face (`viewport_cmd`
        // does nothing outside `Surface::Window`).
        if output.window_drag {
            self.viewport_cmd(ctx, egui::ViewportCommand::StartDrag);
        }
        self.root.last_layout = laid_layout;
        self.root.mirror_layout();

        // 055 — notifications, over the controls and inside this form's own
        // surface (D3/R16). Nothing here resizes anything: the rects were
        // computed inside `snack_surface` and painted on the caller's painter
        // (R26/AC13).
        self.root.draw_snackbars(root_ui, snack_surface);


        // ── Animation triggers from this frame's interaction ─────────────────
        // Pointer triggers come from the rendered rects: the engine only emits
        // onClick/onHoverEnter for controls that have a bound COBOL handler, but
        // an animation is reason enough on its own. Focus and timer triggers do
        // come from the event stream (`onTick` always fires; `onGotFocus` fires
        // when bound).
        if armed {
            let (clicked, pointer) =
                ctx.input(|i| (i.pointer.primary_clicked(), i.pointer.interact_pos()));
            let mut still_hovered = std::collections::HashSet::new();
            for (id, rect) in &output.control_rects {
                // Repeating-group card instances are drawn under a composite id
                // and carry their own placement effect; leave them alone.
                if id.contains('.') {
                    continue;
                }
                let over = pointer.map(|p| rect.contains(p)).unwrap_or(false);
                if over {
                    still_hovered.insert(id.clone());
                    if !self.root.hovered.contains(id) {
                        self.root.anim.fire_event(&self.root.controls, id, "onHoverEnter");
                    }
                    if clicked {
                        self.root.anim.fire_event(&self.root.controls, id, "onClick");
                    }
                }
            }
            self.root.hovered = still_hovered;
            for ev in &output.events {
                // Pointer events are already covered by the rect pass above —
                // taking them from here too would restart the same animation twice.
                if ev.event.eq_ignore_ascii_case("onClick")
                    || ev.event.eq_ignore_ascii_case("onHoverEnter")
                {
                    continue;
                }
                self.root.anim.fire_event(&self.root.controls, &ev.ctrl_id, &ev.event);
            }
        }

        // Apply value updates locally, sync them to the interpreter (so
        // handlers read the live value), and forward UI events — once armed.
        let mut interacted = false;
        if armed {
            // Live values, then the events with Timer-tick coalescing — shared
            // with the child-window path.
            if self
                .root
                .forward_interaction(&output.prop_updates, output.events, root_blocked)
            {
                interacted = true;
            }

            // A FileDropZone's native picker and a toolbar button's platform
            // action — shared with the child-window path.
            if self.root.run_platform_requests(
                ctx,
                &output.file_picker_requests,
                &output.csv_export_requests,
                &output.toolbar_actions,
                pre_focus,
                None,
            ) {

                interacted = true;
            }
        }
        self.root.show_action_notice(ctx);

        // Reactive frame scheduling — never spin at max FPS. While interpreter
        // traffic is flowing (state drained, events sent, or a backlog is
        // queued), poll fast; otherwise a slow heartbeat keeps DISPLAY output
        // and end-of-program detection timely. Timer controls schedule their
        // own precise wake-ups inside the render engine, and user input wakes
        // egui automatically — between all of those, the process sleeps.
        // A running animation needs frames of its own: without this the form
        // sleeps between interpreter traffic and a fly-in would advance in 200 ms
        // jumps (or freeze mid-flight on an idle form).
        let busy = drained > 0
            || interacted
            || animating
            || self.root.anim.is_animating()
            || self.root.pending.load(Ordering::Relaxed) > 0;
        let ms = if busy { 16 } else { 200 };
        ctx.request_repaint_after(std::time::Duration::from_millis(ms));
    }
}

// ── Parity suite (spec 042 R29) ───────────────────────────────────────────────
// ONE host means testing it once tests every surface. These tests drive
// `FormHost` headlessly (`Context::run_ui`, texture deltas cleared per the
// egui 0.36 idiom) and assert at the DECISION level: which viewport commands
// the host emits, which events it sends, which gates open when. True OS-window
// realities (real transparency, decorations, monitors) are the operator's
// manual pass — stated in `zz_parity_report`.
#[cfg(test)]
mod closed_fanout_tests {
    use super::*;
    use std::sync::mpsc;

    /// 051 / 037 R24 — one close reaches EVERY registered interpreter, and a
    /// dead receiver silences nothing for the live ones.
    #[test]
    fn one_close_reaches_every_interpreter() {
        let (root_tx, root_rx) = mpsc::channel();
        let mut fanout = ClosedFanout::new(root_tx);
        let (a_tx, a_rx) = mpsc::channel();
        let (b_tx, b_rx) = mpsc::channel();
        fanout.register(a_tx);
        fanout.register(b_tx);

        fanout.send("W3");
        let got: Vec<String> = [&root_rx, &a_rx, &b_rx]
            .iter()
            .map(|rx| rx.try_recv().expect("delivered"))
            .collect();
        assert_eq!(got, vec!["W3", "W3", "W3"]);

        // An interpreter that already ended (receiver dropped) is skipped;
        // the survivors still hear the next close.
        drop(a_rx);
        fanout.send("W4");
        assert_eq!(root_rx.try_recv().unwrap(), "W4");
        assert_eq!(b_rx.try_recv().unwrap(), "W4");

        println!("fan-out: 1 close × 3 receivers = 3 deliveries; dead receiver skipped, 2/2 survivors still served");
    }
}

#[cfg(test)]
mod multi_form_tests {
    use super::*;
    use std::sync::mpsc;

    /// A tiny real program for a spawned child: it stops at once, which
    /// exercises spawn → run → finished → release in one pass.
    fn child_program() -> cobolt_ast::program::Program {
        let src = "\
IDENTIFICATION DIVISION.\nPROGRAM-ID. CHILD.\nPROCEDURE DIVISION.\n    STOP RUN.\n";
        cobolt_parser::parse(cobolt_lexer::tokenize(src, cobolt_lexer::SourceFormat::Free))
            .program
            .expect("child parses")
    }

    fn host_with_source(
        with_source: bool,
    ) -> (FormHost, mpsc::Receiver<String>, mpsc::Sender<cobolt_runtime::form_host::FormRequest>)
    {
        let form = cobolt_forms::Form::new("MAIN-FORM", "Main", 320, 200);
        let (ev_tx, _ev_rx) = mpsc::channel();
        let (input_tx, _input_rx) = mpsc::channel();
        let (_state_tx, state_rx) = mpsc::channel();
        let (_display_tx, display_rx) = mpsc::channel();
        let (form_req_tx, form_req_rx) = mpsc::channel();
        let (closed_tx, closed_rx) = mpsc::channel();
        let source: Option<FormSource> = with_source.then(|| -> FormSource {
            Box::new(|id: &str| {
                if id.eq_ignore_ascii_case("DETAIL") {
                    Ok((
                        cobolt_forms::Form::new("DETAIL", "Detail", 240, 160),
                        child_program(),
                    ))
                } else {
                    Err(format!("no form named '{id}'"))
                }
            })
        });
        let (host, _form) = FormHost::new(FormHostConfig {
            form,
            flat: Vec::new(),
            state: HashMap::new(),
            ev_tx,
            input_tx,
            state_rx,
            display_rx,
            pending: Arc::new(AtomicUsize::new(0)),
            finished: Arc::new(AtomicBool::new(false)),
            form_req_rx,
            closed_tx,
            form_req_tx: form_req_tx.clone(),
            form_source: source,
            child_theme: None,
            child_interpreter_setup: None,
            indexed_engine: Default::default(),
            shared_rust_bridge: None,
            fx_entrance: cobolt_forms::window_fx::FxSpec::default(),
            fx_exit: cobolt_forms::window_fx::FxSpec::default(),
            fx_restore: false,
            theme_pack: None,
            surface_theme: cobolt_forms::surface_theme::liquid_glass(),
            icon_path: None,
            title_fallback: String::new(),
            hooks: Box::new(NoHooks),
            surface: Surface::Window,
        });
        (host, closed_rx, form_req_tx)
    }

    /// A form whose rail is exactly what a shell would open on: one SideMenu,
    /// full height, 200 wide.
    fn form_with_side_menu() -> cobolt_forms::Form {
        let mut form = cobolt_forms::Form::new("DETAIL", "Detail", 640, 420);
        let mut side =
            cobolt_forms::Control::new("SideMenu-1", cobolt_forms::ControlType::SideMenu, 0, 0);
        side.rect = cobolt_forms::model::Rect::new(0, 0, 200, 420);
        form.add_control(side);
        form
    }

    /// A host whose ROOT body carries a SideMenu — the body shape a spawned
    /// child window has when its form is one that would open as a shell.
    fn host_with_side_menu() -> FormHost {
        let (ev_tx, _ev_rx) = mpsc::channel();
        let (input_tx, _input_rx) = mpsc::channel();
        let (_state_tx, state_rx) = mpsc::channel();
        let (_display_tx, display_rx) = mpsc::channel();
        let (form_req_tx, form_req_rx) = mpsc::channel();
        let (closed_tx, _closed_rx) = mpsc::channel();
        let form = form_with_side_menu();
        // The body renders from the FLATTENED list, which is what every real
        // caller passes; an empty one would give the body no controls at all.
        let mut flat = Vec::new();
        crate::flatten_controls(&form.controls, &mut flat);
        let (host, _form) = FormHost::new(FormHostConfig {
            form,
            flat,
            state: HashMap::new(),
            ev_tx,
            input_tx,
            state_rx,
            display_rx,
            pending: Arc::new(AtomicUsize::new(0)),
            finished: Arc::new(AtomicBool::new(false)),
            form_req_rx,
            closed_tx,
            form_req_tx,
            form_source: None,
            child_theme: None,
            child_interpreter_setup: None,
            indexed_engine: Default::default(),
            shared_rust_bridge: None,
            fx_entrance: cobolt_forms::window_fx::FxSpec::default(),
            fx_exit: cobolt_forms::window_fx::FxSpec::default(),
            fx_restore: false,
            theme_pack: None,
            surface_theme: cobolt_forms::surface_theme::liquid_glass(),
            icon_path: None,
            title_fallback: String::new(),
            hooks: Box::new(NoHooks),
            surface: Surface::Window,
        });
        host
    }

    /// 049/051 — a form with a SideMenu running in its own WINDOW draws the
    /// breadcrumb strip itself, so the rail keeps its Open/Collapsed control.
    ///
    /// Spawned as a child window by another form's sidebar, such a form used to
    /// get no chrome at all (`child_frame` was handed `None`) and the rail lost
    /// the fold/unfold button it has on every other surface — the designer
    /// canvas, the preview and `rcrun run-form`, which opens the same form as a
    /// shell (operator, 2026-09-10).
    #[test]
    fn a_window_with_a_side_menu_draws_its_own_fold_control() {
        let mut host = host_with_side_menu();
        let ctx = egui::Context::default();
        let window = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(640.0, 420.0));

        // Frame 1: lay the strip out and find the toggle.
        let mut toggle = egui::Rect::NOTHING;
        let mut painted = false;
        ctx.run_ui(egui::RawInput::default(), |ui| {
            let chrome = host.root.window_crumb_chrome(ui, window, "Detail");
            painted = chrome.is_some();
            if let Some(c) = &chrome {
                c(ui.painter(), window);
            }
            toggle = host.root.last_window_crumb.as_ref().unwrap().toggle;
        })
        .textures_delta
        .clear();

        assert!(painted, "a form with a SideMenu must draw its own strip");
        assert!(
            toggle.width() > 4.0 && toggle.height() > 4.0,
            "the fold control must be a real target, got {toggle:?}"
        );
        assert!(
            window.contains(toggle.center()),
            "the fold control must sit inside the window, got {toggle:?}"
        );
        assert!(
            !host.root.side_menu_collapsed("SideMenu-1"),
            "the rail starts open"
        );

        // Click it, exactly where it was drawn. Press and release are separate
        // frames, and the toggle is registered on every one of them: egui hit-
        // tests a press against the widgets the PREVIOUS frame declared, so a
        // control that appears only on the frame of the press is never pressed.
        let click = toggle.center();
        let button = |pressed: bool| egui::RawInput {
            events: vec![
                egui::Event::PointerMoved(click),
                egui::Event::PointerButton {
                    pos: click,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::default(),
                },
            ],
            ..Default::default()
        };
        for input in [
            egui::RawInput {
                events: vec![egui::Event::PointerMoved(click)],
                ..Default::default()
            },
            button(true),
            button(false),
        ] {
            ctx.run_ui(input, |ui| {
                let _ = host.root.window_crumb_chrome(ui, window, "Detail");
                host.root.window_crumb_interact(ui);
            })
            .textures_delta
            .clear();
        }

        assert!(
            host.root.side_menu_collapsed("SideMenu-1"),
            "clicking the fold control must collapse the rail"
        );

        println!(
            "049/051 window fold control — strip laid out, toggle {:.0}x{:.0} at ({:.0}, {:.0}), \
             one click folded the rail",
            toggle.width(),
            toggle.height(),
            toggle.center().x,
            toggle.center().y
        );
    }

    /// A child window whose rail is folded PAINTS it folded: the rail at its
    /// collapsed width and the content slid left with it — the same picture the
    /// shell, the designer and the preview give. It used to paint the designed
    /// rect: a full-width rail of icon-only rows with the breadcrumb under it.
    #[test]
    fn a_child_window_paints_a_folded_rail_at_its_collapsed_width() {
        let mut host = host_with_side_menu();
        let mut label =
            cobolt_forms::Control::new("Lbl-1", cobolt_forms::ControlType::Label, 0, 0);
        label.rect = cobolt_forms::model::Rect::new(260, 40, 200, 30);
        host.root.controls.push(label);
        let side = host.root.controls.iter().find(|c| c.id == "SideMenu-1").unwrap().clone();
        let folded_w = cobolt_forms::sidebar::shown_width(&side, true) as i32;

        let open = host.root.painted_controls();
        let rect_of = |cs: &[cobolt_forms::Control], id: &str| {
            cs.iter().find(|c| c.id == id).unwrap().rect
        };
        assert_eq!(rect_of(&open, "SideMenu-1").w, 200, "open: the designed width");
        assert_eq!(rect_of(&open, "Lbl-1").x, 260, "open: content where it was drawn");

        host.root.state.insert(
            "SideMenu-1".into(),
            crate::state::CtrlState {
                props: [("Collapsed".to_string(), "true".to_string())].into(),
                visible: true,
                enabled: true,
            },
        );
        let folded = host.root.painted_controls();
        assert_eq!(rect_of(&folded, "SideMenu-1").w, folded_w, "folded: the collapsed width");
        assert_eq!(
            rect_of(&folded, "Lbl-1").x,
            260 - (200 - folded_w),
            "folded: content slides left by the width the rail gave up"
        );
        assert_eq!(
            host.root.controls.iter().find(|c| c.id == "SideMenu-1").unwrap().rect.w,
            200,
            "the design itself is never edited"
        );
        println!(
            "child window rail — open 200 px, folded {folded_w} px; label x 260 -> {}",
            260 - (200 - folded_w)
        );
    }

    /// …and a form with no SideMenu produces no strip at all, so an ordinary
    /// child window is untouched by any of this.
    #[test]
    fn a_window_without_a_side_menu_draws_no_strip() {
        let (mut host, _closed, _req) = host_with_source(false);
        let ctx = egui::Context::default();
        let window = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(320.0, 200.0));
        let mut chrome_some = false;
        ctx.run_ui(egui::RawInput::default(), |ui| {
            chrome_some = host.root.window_crumb_chrome(ui, window, "Main").is_some();
        })
        .textures_delta
        .clear();
        assert!(!chrome_some, "no SideMenu, no strip");
        assert!(host.root.last_window_crumb.is_none());
    }

    /// A program that stays open: one event loop, forever.
    fn waiting_program(id: &str) -> cobolt_ast::program::Program {
        program_from(&format!(
            "IDENTIFICATION DIVISION.\nPROGRAM-ID. {id}.\n\
             DATA DIVISION.\nWORKING-STORAGE SECTION.\n\
             01 EVT PIC X(30).\n01 CTL PIC X(30).\n\
             PROCEDURE DIVISION.\n    \
             PERFORM UNTIL 1 = 2\n        \
             CALL \"COBOL-WAIT-EVENT\" USING EVT CTL\n    \
             END-PERFORM.\n"
        ))
    }

    /// A main window whose form source knows `APP-SHELL` (a form with its own
    /// SideMenu, 200 wide) and `PAGE-ONE` (an Embedded screen); both stay open.
    fn host_with_a_shell_form() -> FormHost {
        let form = cobolt_forms::Form::new("MAIN-FORM", "Main", 320, 200);
        let (ev_tx, _ev_rx) = mpsc::channel();
        let (input_tx, _input_rx) = mpsc::channel();
        let (_state_tx, state_rx) = mpsc::channel();
        let (_display_tx, display_rx) = mpsc::channel();
        let (form_req_tx, form_req_rx) = mpsc::channel();
        let (closed_tx, _closed_rx) = mpsc::channel();
        let source: Option<FormSource> = Some(Box::new(|id: &str| {
            if id.eq_ignore_ascii_case("APP-SHELL") {
                let mut form = cobolt_forms::Form::new("APP-SHELL", "Assistant", 640, 420);
                let mut side =
                    cobolt_forms::Control::new("SideMenu-1", cobolt_forms::ControlType::SideMenu, 0, 0);
                side.rect = cobolt_forms::model::Rect::new(0, 0, 200, 420);
                form.add_control(side);
                let mut label = cobolt_forms::Control::new("LBL-HOME", cobolt_forms::ControlType::Label, 240, 60);
                label.rect = cobolt_forms::model::Rect::new(240, 60, 120, 24);
                form.add_control(label);
                Ok((form, waiting_program("APP-SHELL")))
            } else if id.eq_ignore_ascii_case("PAGE-ONE") {
                Ok((cobolt_forms::Form::new("PAGE-ONE", "Topics", 440, 420), waiting_program("PAGE-ONE")))
            } else {
                Err(format!("no form named '{id}'"))
            }
        }));
        let (host, _form) = FormHost::new(FormHostConfig {
            form,
            flat: Vec::new(),
            state: HashMap::new(),
            ev_tx,
            input_tx,
            state_rx,
            display_rx,
            pending: Arc::new(AtomicUsize::new(0)),
            finished: Arc::new(AtomicBool::new(false)),
            form_req_rx,
            closed_tx,
            form_req_tx,
            form_source: source,
            child_theme: None,
            child_interpreter_setup: None,
            indexed_engine: Default::default(),
            shared_rust_bridge: None,
            fx_entrance: cobolt_forms::window_fx::FxSpec::default(),
            fx_exit: cobolt_forms::window_fx::FxSpec::default(),
            fx_restore: false,
            theme_pack: None,
            surface_theme: cobolt_forms::surface_theme::liquid_glass(),
            icon_path: None,
            title_fallback: String::new(),
            hooks: Box::new(NoHooks),
            surface: Surface::Window,
        });
        host
    }

    /// Spec 085 AC1 (R1–R3, R5) — a form with its own SideMenu, opened as a
    /// window, runs as a shell of its own: the rail is chrome, not a control
    /// on its form; a menu item loads a form into THAT window's ContentPane
    /// (never the main window's), with the window's form as its caller, and
    /// the breadcrumb follows; Home brings the window's own form back; closing
    /// the window releases the form loaded into it.
    #[test]
    fn a_side_menu_form_opened_as_a_window_runs_as_a_shell_of_its_own() {
        let mut host = host_with_a_shell_form();
        let ctx = egui::Context::default();
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0)));
        let frame = |host: &mut FormHost| {
            let mut f = ctx.run_ui(input.clone(), |ui| {
                let c = ui.ctx().clone();
                host.update_children(&c);
            });
            f.textures_delta.clear();
        };
        let w = host.supervisor_open_for_test("APP-SHELL");
        let mut f = ctx.run_ui(input.clone(), |ui| {
            host.apply_host_actions(ui.ctx(), vec![spawn_action(&w, "APP-SHELL")]);
        });
        f.textures_delta.clear();
        assert_eq!(host.children.len(), 1);
        let child = &host.children[0];
        assert!(child.nav.is_some() && child.pane.is_some(), "the window runs as a shell");
        assert!(
            !child.body.controls.iter().any(|c| c.control_type == cobolt_forms::ControlType::SideMenu),
            "the rail is the shell's chrome, not a control on the form"
        );
        let label = child.body.controls.iter().find(|c| c.id == "LBL-HOME").unwrap();
        assert_eq!(label.rect.x, 40, "the content slid over the rail's 200-point column");
        assert_eq!(child.size, egui::vec2(640.0, 420.0), "the window opens at the designed size");
        frame(&mut host);

        // A menu item of THAT window loads a form into ITS pane.
        host.children[0].nav.as_mut().unwrap().shell.queue_click(crate::shell::MenuClick {
            slot: crate::shell::MenuSlot::Root,
            item_id: "tpcs".into(),
            action: Some("open-form:page-one".into()),
            preserve_previous_form: false,
        });
        frame(&mut host);
        frame(&mut host);
        let child = &host.children[0];
        let pane = child.pane.as_ref().unwrap();
        assert_eq!(pane.active_occupant.as_deref(), Some("PAGE-ONE"), "loaded into the window's own pane");
        assert!(host.pane.occupants.is_empty(), "nothing loaded into the main window's pane");
        let occupant_handle = pane.occupants["PAGE-ONE"].handle.clone();
        assert_eq!(host.supervisor.caller_of(&occupant_handle), Some(w.as_str()), "super:: is the window's form");
        let crumbs: Vec<String> =
            child.nav.as_ref().unwrap().chain.segments().into_iter().map(|(_, l)| l).collect();
        assert_eq!(crumbs, ["Assistant", "Topics"], "the window's breadcrumb follows its chain");

        // Home: the window's own form is back on its pane; the loaded form is
        // parked, still resident.
        host.children[0].nav.as_mut().unwrap().shell.queue_click(crate::shell::MenuClick {
            slot: crate::shell::MenuSlot::Root,
            item_id: "home".into(),
            action: Some("home".into()),
            preserve_previous_form: false,
        });
        frame(&mut host);
        let pane = host.children[0].pane.as_ref().unwrap();
        assert_eq!(pane.active_occupant, None, "Home shows the window's own form");
        assert!(pane.occupants.contains_key("PAGE-ONE"), "parked, not destroyed");

        // Closing the window releases the form loaded into it.
        let mut f = ctx.run_ui(input.clone(), |ui| {
            host.apply_host_actions(
                ui.ctx(),
                vec![cobolt_runtime::form_host::HostAction::CloseWindow { handle: w.clone() }],
            );
        });
        f.textures_delta.clear();
        assert!(host.children.is_empty());
        assert!(host.supervisor.caller_of(&occupant_handle).is_none(), "the loaded form's handle is released");
        println!(
            "child shell: {w} opened as a shell (rail out of the form, content slid 200 pt), \
             PAGE-ONE loaded into its own pane with super = {w}, breadcrumb Assistant > Topics, \
             Home parked it, closing released it"
        );
    }

    fn spawn_action(handle: &str, form: &str) -> cobolt_runtime::form_host::HostAction {
        cobolt_runtime::form_host::HostAction::SpawnWindow {
            handle: handle.into(),
            form_id: form.into(),
            window_state: None,
            x: None,
            y: None,
            width: None,
            height: None,
            modal: false,
        }
    }

    /// 051 R6/R3 — a spawn builds a real child (own body, own interpreter);
    /// its STOP RUN releases the handle through the supervisor and the close
    /// reaches the fan-out (R8). One test, the whole child lifecycle.
    #[test]
    fn spawn_runs_a_child_to_completion_and_releases_it() {
        let (mut host, closed_rx, _req_tx) = host_with_source(true);
        let ctx = egui::Context::default();
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::Vec2::new(640.0, 480.0),
        ));
        // The supervisor allocated W1 for this open elsewhere; here we drive
        // the ACTION arm directly, headlessly.
        host.supervisor_open_for_test("DETAIL");
        let mut full = ctx.run_ui(input.clone(), |ui| {
            host.apply_host_actions(ui.ctx(), vec![spawn_action("W1", "DETAIL")]);
        });
        full.textures_delta.clear();
        assert_eq!(host.children.len(), 1, "the child window exists");
        assert_eq!(host.children[0].handle, "W1");
        assert_eq!(host.children[0].body.form_name, "DETAIL");
        assert!(
            !host.children[0].body.controls.is_empty() || host.children[0].body.state.is_empty(),
            "the body is built from the child's own design"
        );

        // The child's program is `STOP RUN` — drive frames until its finish
        // is reported and the window released (bounded wait).
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !host.children.is_empty() && std::time::Instant::now() < deadline {
            let mut f = ctx.run_ui(input.clone(), |ui| {
                let c = ui.ctx().clone();
                host.update_children(&c);
            });
            f.textures_delta.clear();
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(host.children.is_empty(), "STOP RUN released the child");
        let closed: Vec<String> = closed_rx.try_iter().collect();
        assert!(
            closed.contains(&"W1".to_string()),
            "NotifyClosed reached the fan-out: {closed:?}"
        );
        println!(
            "child spawn — W1 built (form DETAIL), ran to STOP RUN, released; \
             NotifyClosed delivered: {closed:?}"
        );
    }

    /// 051 R19/R28 — the click-through mitigation's decision logic:
    /// `live_modal_caller_viewport` must resolve a LIVE MODAL child to its
    /// caller's viewport (here, root), and forget it once the child stops
    /// being a live modal (closed / released). `update_children` uses this
    /// exact lookup, each frame, to decide (a) whether the child's viewport
    /// should be built always-on-top and (b) whether to steal focus back
    /// when the caller's OWN viewport reports focused (i.e. the operator
    /// clicked the "blocked" caller and the OS actually raised it — which
    /// `disable()` + the overlay paint can never prevent on their own; see
    /// the long comment at the call site for why no real OS-level modal
    /// parenting is reachable through this egui/eframe/winit version).
    ///
    /// This does NOT drive `update_children` end-to-end and assert on
    /// `FullOutput::viewport_output`: `show_viewport_immediate` only
    /// populates that reliably under a real `IMMEDIATE_VIEWPORT_RENDERER`,
    /// which only a live eframe runtime installs — under
    /// `egui::Context::default()` in a unit test it silently falls back to
    /// `show_embedded_viewport`, so the command never lands where the test
    /// could see it even though `send_viewport_cmd_to` fired correctly.
    /// Testing the pure decision function is what's actually reachable
    /// headlessly, and it's the one place the reactive-refocus logic can go
    /// wrong.
    #[test]
    fn live_modal_caller_viewport_finds_the_focused_callers_viewport() {
        // A CHILD that stays open (an event loop, not a bare STOP RUN) —
        // the test needs it alive across several frames.
        let (mut host, _closed_rx, _req_tx) = {
            let form = cobolt_forms::Form::new("MAIN-FORM", "Main", 320, 200);
            let (ev_tx, _ev_rx) = mpsc::channel();
            let (input_tx, _input_rx) = mpsc::channel();
            let (_state_tx, state_rx) = mpsc::channel();
            let (_display_tx, display_rx) = mpsc::channel();
            let (form_req_tx, form_req_rx) = mpsc::channel();
            let (closed_tx, closed_rx) = mpsc::channel();
            let source: Option<FormSource> = Some(Box::new(|id: &str| {
                if id.eq_ignore_ascii_case("DETAIL") {
                    Ok((
                        cobolt_forms::Form::new("DETAIL", "Detail", 240, 160),
                        program_from(
                            "IDENTIFICATION DIVISION.\nPROGRAM-ID. DETAIL.\n\
                             DATA DIVISION.\nWORKING-STORAGE SECTION.\n\
                             01 EVT PIC X(30).\n01 CTL PIC X(30).\n\
                             PROCEDURE DIVISION.\n    \
                             PERFORM UNTIL 1 = 2\n        \
                             CALL \"COBOL-WAIT-EVENT\" USING EVT CTL\n    \
                             END-PERFORM.\n",
                        ),
                    ))
                } else {
                    Err(format!("no form named '{id}'"))
                }
            }));
            let (host, _form) = FormHost::new(FormHostConfig {
                form,
                flat: Vec::new(),
                state: HashMap::new(),
                ev_tx,
                input_tx,
                state_rx,
                display_rx,
                pending: Arc::new(AtomicUsize::new(0)),
                finished: Arc::new(AtomicBool::new(false)),
                form_req_rx,
                closed_tx,
                form_req_tx: form_req_tx.clone(),
                form_source: source,
                child_theme: None,
                child_interpreter_setup: None,
                indexed_engine: Default::default(),
                shared_rust_bridge: None,
                fx_entrance: cobolt_forms::window_fx::FxSpec::default(),
                fx_exit: cobolt_forms::window_fx::FxSpec::default(),
                fx_restore: false,
                theme_pack: None,
                surface_theme: cobolt_forms::surface_theme::liquid_glass(),
                icon_path: None,
                title_fallback: String::new(),
                hooks: Box::new(NoHooks),
                surface: Surface::Window,
            });
            (host, closed_rx, form_req_tx)
        };
        let ctx = egui::Context::default();
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::Vec2::new(640.0, 480.0),
        ));

        // Register a MODAL open (supervisor_open_for_test is non-modal), then
        // build the child window the same way the real spawn action does.
        let (reply_tx, _reply_rx) = mpsc::channel();
        let _ = host
            .supervisor
            .handle_request(cobolt_runtime::form_host::FormRequest::OpenForm {
                caller: cobolt_runtime::form_host::ROOT_HANDLE.into(),
                form_id: "DETAIL".into(),
                sync: true,
                window_state: None,
                x: None,
                y: None,
                width: None,
                height: None,
                modal: true,
                reply: reply_tx,
            });
        let mut full = ctx.run_ui(input.clone(), |ui| {
            host.apply_host_actions(ui.ctx(), vec![spawn_action("W1", "DETAIL")]);
        });
        full.textures_delta.clear();
        assert_eq!(host.children[0].handle, "W1");

        // W1 IS the registered modal child of ROOT — the lookup must find
        // root's viewport as the one to watch.
        assert_eq!(
            host.live_modal_caller_viewport("W1"),
            Some(egui::ViewportId::ROOT),
            "a live modal child of root resolves to root's own viewport"
        );

        // Drive one real `update_children` frame with root reporting
        // focused — a smoke test that the production call site actually
        // reads this decision and queues the command (via
        // `ctx.send_viewport_cmd_to`) without panicking; the command's
        // delivery into `FullOutput` is NOT observable here (see the doc
        // comment above), so this only guards against the call site
        // diverging from the decision function, not the OS-level effect.
        input
            .viewports
            .insert(egui::ViewportId::ROOT, egui::ViewportInfo {
                focused: Some(true),
                ..Default::default()
            });
        let mut full = ctx.run_ui(input.clone(), |ui| {
            let c = ui.ctx().clone();
            host.update_children(&c);
        });
        full.textures_delta.clear();

        // Once W1 is no longer a live modal (its supervisor entry released
        // via the ordinary close path), the lookup must stop pointing at
        // root — otherwise a closed/reused handle would keep stealing focus
        // forever.
        host.supervisor.try_close("W1");
        assert_eq!(
            host.live_modal_caller_viewport("W1"),
            None,
            "a released child is no longer a live modal of anything"
        );
    }

    fn program_from(src: &str) -> cobolt_ast::program::Program {
        cobolt_parser::parse(cobolt_lexer::tokenize(src, cobolt_lexer::SourceFormat::Free))
            .program
            .expect("parses")
    }

    /// A host whose `FormSource` resolves TWO forms: CALLER (a trivial
    /// ContentPane occupant) and CHILD (a Sync window CALLER opens, whose
    /// whole program is a `super::"SetProperty"` probe — it can only reach
    /// `DISPLAY "SUPER-OK"` if its `super` is bound).
    fn host_with_caller_and_child(
    ) -> (FormHost, mpsc::Receiver<String>, mpsc::Sender<cobolt_runtime::form_host::FormRequest>)
    {
        let form = cobolt_forms::Form::new("MAIN-FORM", "Main", 320, 200);
        let (ev_tx, _ev_rx) = mpsc::channel();
        let (input_tx, _input_rx) = mpsc::channel();
        let (_state_tx, state_rx) = mpsc::channel();
        let (_display_tx, display_rx) = mpsc::channel();
        let (form_req_tx, form_req_rx) = mpsc::channel();
        let (closed_tx, closed_rx) = mpsc::channel();
        let source: Option<FormSource> = Some(Box::new(|id: &str| {
            if id.eq_ignore_ascii_case("CALLER") {
                Ok((
                    cobolt_forms::Form::new("CALLER", "Caller", 320, 200),
                    program_from(
                        "IDENTIFICATION DIVISION.\nPROGRAM-ID. CALLER.\n\
                         PROCEDURE DIVISION.\n    STOP RUN.\n",
                    ),
                ))
            } else if id.eq_ignore_ascii_case("CHILD") {
                Ok((
                    cobolt_forms::Form::new("CHILD", "Child", 240, 160),
                    program_from(
                        "IDENTIFICATION DIVISION.\nPROGRAM-ID. CHILD.\n\
                         PROCEDURE DIVISION.\n    \
                         INVOKE SUPER::\"SetProperty\"(\"Probe\", \"ok\").\n    \
                         DISPLAY \"SUPER-OK\".\n    STOP RUN.\n",
                    ),
                ))
            } else {
                Err(format!("no form named '{id}'"))
            }
        }));
        let (host, _form) = FormHost::new(FormHostConfig {
            form,
            flat: Vec::new(),
            state: HashMap::new(),
            ev_tx,
            input_tx,
            state_rx,
            display_rx,
            pending: Arc::new(AtomicUsize::new(0)),
            finished: Arc::new(AtomicBool::new(false)),
            form_req_rx,
            closed_tx,
            form_req_tx: form_req_tx.clone(),
            form_source: source,
            child_theme: None,
            child_interpreter_setup: None,
            indexed_engine: Default::default(),
            shared_rust_bridge: None,
            fx_entrance: cobolt_forms::window_fx::FxSpec::default(),
            fx_exit: cobolt_forms::window_fx::FxSpec::default(),
            fx_restore: false,
            theme_pack: None,
            surface_theme: cobolt_forms::surface_theme::liquid_glass(),
            icon_path: None,
            title_fallback: String::new(),
            hooks: Box::new(NoHooks),
            surface: Surface::Window,
        });
        (host, closed_rx, form_req_tx)
    }

    /// A child form creates indexed files with the engine the host was given —
    /// the one `rcrun run-form --indexed-engine` / the project setting gave the
    /// ROOT interpreter. Every child used to be built with the runtime default,
    /// so a project set to redb wrote redb files from its main form and
    /// PRCIDXD1 files from every form it opened.
    #[test]
    fn a_child_form_creates_indexed_files_with_the_hosts_engine() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("child.idx");
        let path_lit = path.to_string_lossy().replace('"', "");
        let src = format!(
            "IDENTIFICATION DIVISION.\nPROGRAM-ID. CHILD.\n\
             ENVIRONMENT DIVISION.\nINPUT-OUTPUT SECTION.\nFILE-CONTROL.\n    \
             SELECT IDX-FILE ASSIGN TO \"{path_lit}\"\n        \
             ORGANIZATION IS INDEXED\n        ACCESS MODE IS DYNAMIC\n        \
             RECORD KEY IS IDX-KEY.\n\
             DATA DIVISION.\nFILE SECTION.\nFD IDX-FILE.\n01 IDX-REC.\n    \
             05 IDX-KEY PIC X(4).\n    05 IDX-DATA PIC X(10).\n\
             PROCEDURE DIVISION.\n    OPEN OUTPUT IDX-FILE.\n    \
             MOVE \"0001\" TO IDX-KEY.\n    MOVE \"CHILD\" TO IDX-DATA.\n    \
             WRITE IDX-REC.\n    CLOSE IDX-FILE.\n    STOP RUN.\n"
        );
        let source: Option<FormSource> = Some(Box::new(move |id: &str| {
            if id.eq_ignore_ascii_case("CHILD") {
                Ok((cobolt_forms::Form::new("CHILD", "Child", 240, 160), program_from(&src)))
            } else {
                Err(format!("no form named '{id}'"))
            }
        }));
        let (ev_tx, _ev_rx) = mpsc::channel();
        let (input_tx, _input_rx) = mpsc::channel();
        let (_state_tx, state_rx) = mpsc::channel();
        let (_display_tx, display_rx) = mpsc::channel();
        let (form_req_tx, form_req_rx) = mpsc::channel();
        let (closed_tx, _closed_rx) = mpsc::channel();
        let (mut host, _form) = FormHost::new(FormHostConfig {
            form: cobolt_forms::Form::new("MAIN-FORM", "Main", 320, 200),
            flat: Vec::new(),
            state: HashMap::new(),
            ev_tx,
            input_tx,
            state_rx,
            display_rx,
            pending: Arc::new(AtomicUsize::new(0)),
            finished: Arc::new(AtomicBool::new(false)),
            form_req_rx,
            closed_tx,
            form_req_tx,
            form_source: source,
            child_theme: None,
            child_interpreter_setup: None,
            indexed_engine: cobolt_runtime::indexed::IndexedEngine::Redb,
            shared_rust_bridge: None,
            fx_entrance: cobolt_forms::window_fx::FxSpec::default(),
            fx_exit: cobolt_forms::window_fx::FxSpec::default(),
            fx_restore: false,
            theme_pack: None,
            surface_theme: cobolt_forms::surface_theme::liquid_glass(),
            icon_path: None,
            title_fallback: String::new(),
            hooks: Box::new(NoHooks),
            surface: Surface::Window,
        });
        // The same spawn path a child window and a modal child take.
        host.ensure_occupant("CHILD").expect("occupant builds");

        // The child's program runs on its own thread; wait for its file.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        let magic = loop {
            if let Ok(bytes) = std::fs::read(&path) {
                // Either container's magic — until one is written, the file
                // may still be being laid out.
                if bytes.starts_with(b"redb") || bytes.starts_with(b"PRCIDX") {
                    break bytes[..8.min(bytes.len())].to_vec();
                }
            }
            assert!(std::time::Instant::now() < deadline, "the child never created {path:?}");
            std::thread::sleep(std::time::Duration::from_millis(50));
        };
        assert_eq!(
            &magic[..4],
            b"redb",
            "a child form must create its file with the host's engine (redb), got {:?}",
            String::from_utf8_lossy(&magic)
        );
    }

    /// 049 R28/R29 regression — a child window opened by a ContentPane
    /// OCCUPANT (not the shell/root itself) must still get a bound `super`.
    /// `Interpreter::set_super_form` existed and was covered by
    /// `cobolt-runtime`'s own tests, which call it directly — but nothing in
    /// this crate's real spawn glue ever called it, so `super` was NULL for
    /// EVERY child ever opened in the running application (root-opened or
    /// occupant-opened alike). CALLFORM's demo (PowerDemo3) was the first
    /// form to actually reference `super::` from a form embedded in the
    /// ContentPane, and hit exactly this (operator report, 2026-09-18).
    #[test]
    fn a_child_opened_by_a_contentpane_occupant_gets_a_bound_super() {
        let (mut host, _closed_rx, _req_tx) = host_with_caller_and_child();
        let ctx = egui::Context::default();
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::Vec2::new(640.0, 480.0),
        ));

        // ROOT embeds CALLER into the ContentPane — the same registration
        // the shell's `open-form:` sidebar action performs.
        host.ensure_occupant("CALLER").expect("occupant builds");
        let caller_handle = host.occupant_handle("CALLER").expect("occupant registered");
        assert_ne!(
            caller_handle,
            cobolt_runtime::form_host::ROOT_HANDLE,
            "must be a real occupant handle, not the shell's own — otherwise \
             this test would not distinguish the fix from the bug"
        );

        // CALLER opens CHILD as a Sync/modal window — what
        // `INVOKE me::"OpenFormSync"("CHILD")` does from CALLER's own COBOL;
        // driven directly here since CALLER's program is not under test.
        let (reply_tx, _reply_rx) = mpsc::channel();
        let _ = host
            .supervisor
            .handle_request(cobolt_runtime::form_host::FormRequest::OpenForm {
                caller: caller_handle,
                form_id: "CHILD".into(),
                sync: true,
                window_state: None,
                x: None,
                y: None,
                width: None,
                height: None,
                modal: true,
                reply: reply_tx,
            });
        let child_handle = "W2".to_string(); // W1 = the CALLER occupant above.
        let mut full = ctx.run_ui(input.clone(), |ui| {
            host.apply_host_actions(ui.ctx(), vec![spawn_action(&child_handle, "CHILD")]);
        });
        full.textures_delta.clear();
        let child_idx = host
            .children
            .iter()
            .position(|c| c.handle == child_handle)
            .expect("child window exists");

        // Drive frames until CHILD's program produces its DISPLAY line or
        // its runtime error (bounded wait — either is a terminal outcome).
        // Every frame also drains `form_req_rx`, exactly like the real
        // `ui_impl` loop: CHILD's `INVOKE SUPER::"SetProperty"` is itself a
        // blocking round trip through the supervisor, so without this the
        // request just sits unanswered and the child never gets past it.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            let mut reqs = Vec::new();
            while let Ok(r) = host.form_req_rx.try_recv() {
                reqs.push(r);
            }
            for req in reqs {
                let acts = host.supervisor.handle_request(req);
                let mut f = ctx.run_ui(input.clone(), |ui| {
                    host.apply_host_actions(ui.ctx(), acts.clone());
                });
                f.textures_delta.clear();
            }
            let lines: Vec<String> = host.children[child_idx].body.display_rx.try_iter().collect();
            if !lines.is_empty() {
                assert!(
                    lines.iter().any(|l| l.contains("SUPER-OK")),
                    "expected the child to reach SUPER-OK; got: {lines:?}"
                );
                println!("child display (super bound to a real occupant): {lines:?}");
                return;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "child produced no output within the deadline — still running or hung"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    /// 051 R19/R28 regression — a modal child opened by the ACTIVE OCCUPANT
    /// must block the root exactly as one opened by the root itself does.
    /// `root_modal_blocked` used to check only `modal_children_of(ROOT_HANDLE)`,
    /// so a modal child whose `caller` was an occupant's handle left it
    /// returning `false` — the whole shell face (chrome, breadcrumb, the
    /// ContentPane behind the "modal" window) stayed fully clickable
    /// (operator report, PowerDemo3's Call Form demo, 2026-09-18: "the caller
    /// window become semi transparent, but I still can click on it").
    #[test]
    fn a_modal_child_of_the_active_occupant_blocks_the_root() {
        let (mut host, _closed_rx, _req_tx) = host_with_caller_and_child();
        assert!(!host.root_modal_blocked(), "nothing open yet");

        host.ensure_occupant("CALLER").expect("occupant builds");
        host.show_occupant(Some("CALLER"));
        assert!(
            !host.root_modal_blocked(),
            "an occupant with no modal child of its own must not block"
        );
        let caller_handle = host.occupant_handle("CALLER").expect("occupant registered");

        // CALLER opens CHILD Sync/modal — registration alone is enough here;
        // this test is about the block flag, not the child's own lifecycle.
        let (reply_tx, _reply_rx) = mpsc::channel();
        let _ = host
            .supervisor
            .handle_request(cobolt_runtime::form_host::FormRequest::OpenForm {
                caller: caller_handle,
                form_id: "CHILD".into(),
                sync: true,
                window_state: None,
                x: None,
                y: None,
                width: None,
                height: None,
                modal: true,
                reply: reply_tx,
            });

        assert!(
            host.root_modal_blocked(),
            "a modal child of the ACTIVE OCCUPANT must block the root — \
             this is the exact case that shipped broken"
        );

        // And once the pane shows something else (or nothing), the stale
        // occupant's modal child must NOT keep blocking a root it no longer
        // fronts for.
        host.show_occupant(None);
        assert!(
            !host.root_modal_blocked(),
            "the root must not stay blocked by a modal child of an occupant \
             that is no longer the active one"
        );
    }

    /// A host whose `FormSource` resolves CALLER (a ContentPane occupant with
    /// its OWN event loop — one real `COBOL-WAIT-EVENT` pass after the modal
    /// closes, driven by injecting a second event, exactly as a real form's
    /// generated event loop would be by a real subsequent UI event) and
    /// CHILD (a Sync/modal window it opens, whose whole program publishes a
    /// custom property to its opener through `super::"SetProperty"`).
    fn host_with_publishing_caller_and_child(
    ) -> (FormHost, mpsc::Receiver<String>, mpsc::Sender<cobolt_runtime::form_host::FormRequest>)
    {
        let form = cobolt_forms::Form::new("MAIN-FORM", "Main", 320, 200);
        let (ev_tx, _ev_rx) = mpsc::channel();
        let (input_tx, _input_rx) = mpsc::channel();
        let (_state_tx, state_rx) = mpsc::channel();
        let (display_tx, display_rx) = mpsc::channel();
        let (form_req_tx, form_req_rx) = mpsc::channel();
        let (closed_tx, closed_rx) = mpsc::channel();
        let source: Option<FormSource> = Some(Box::new(|id: &str| {
            if id.eq_ignore_ascii_case("CALLER") {
                let mut form = cobolt_forms::Form::new("CALLER", "Caller", 320, 200);
                let lbl =
                    cobolt_forms::Control::new("Lbl-1", cobolt_forms::ControlType::Label, 10, 10);
                form.add_control(lbl);
                Ok((
                    form,
                    program_from(
                        "IDENTIFICATION DIVISION.\nPROGRAM-ID. CALLER.\n\
                         DATA DIVISION.\nWORKING-STORAGE SECTION.\n\
                         01 WS-H PIC X(8).\n01 WS-RESULT PIC X(40).\n\
                         01 EVT PIC X(30).\n01 CTL PIC X(30).\n\
                         PROCEDURE DIVISION.\n    \
                         INVOKE ME::\"OpenFormSync\"(\"CHILD\") RETURNING WS-H.\n    \
                         CALL \"COBOL-WAIT-EVENT\" USING EVT CTL.\n    \
                         INVOKE ME::\"GetProperty\"(\"Probe\") RETURNING WS-RESULT.\n    \
                         MOVE WS-RESULT TO Lbl-1::Caption.\n    STOP RUN.\n",
                    ),
                ))
            } else if id.eq_ignore_ascii_case("CHILD") {
                Ok((
                    cobolt_forms::Form::new("CHILD", "Child", 240, 160),
                    program_from(
                        "IDENTIFICATION DIVISION.\nPROGRAM-ID. CHILD.\n\
                         PROCEDURE DIVISION.\n    \
                         INVOKE SUPER::\"SetProperty\"(\"Probe\", \"ButtonOk clicked\").\n    \
                         INVOKE ME::\"Close\"().\n    STOP RUN.\n",
                    ),
                ))
            } else {
                Err(format!("no form named '{id}'"))
            }
        }));
        let (host, _form) = FormHost::new(FormHostConfig {
            form,
            flat: Vec::new(),
            state: HashMap::new(),
            ev_tx,
            input_tx,
            state_rx,
            display_rx,
            pending: Arc::new(AtomicUsize::new(0)),
            finished: Arc::new(AtomicBool::new(false)),
            form_req_rx,
            closed_tx,
            form_req_tx: form_req_tx.clone(),
            form_source: source,
            child_theme: None,
            child_interpreter_setup: None,
            indexed_engine: Default::default(),
            shared_rust_bridge: None,
            fx_entrance: cobolt_forms::window_fx::FxSpec::default(),
            fx_exit: cobolt_forms::window_fx::FxSpec::default(),
            fx_restore: false,
            theme_pack: None,
            surface_theme: cobolt_forms::surface_theme::liquid_glass(),
            icon_path: None,
            title_fallback: String::new(),
            hooks: Box::new(NoHooks),
            surface: Surface::Window,
        });
        (host, closed_rx, form_req_tx)
    }

    /// 049/051 regression — `HostAction::SetFormProperty`'s target lookup
    /// only checked `ROOT_HANDLE` and `self.children` (real windows); it
    /// never checked `self.pane.occupants`. A `super::"SetProperty"` write aimed
    /// at a form embedded in the ContentPane (e.g. opened by the sidebar's
    /// `open-form:` action) was accepted by the supervisor — a windowHandle
    /// `GetProperty` on that handle saw it — but silently never reached
    /// THAT form's own interpreter, so its own `me::"GetProperty"` never saw
    /// what a modal child it opened had just written (operator report,
    /// PowerDemo3's Call Form demo, 2026-09-18: "Result from the called
    /// form" never updates). Found with a full end-to-end reproduction using
    /// the demo's real generated COBOL before being reduced to this minimal,
    /// self-contained regression.
    #[test]
    fn a_property_a_child_publishes_to_an_occupant_reaches_that_occupants_own_interpreter() {
        let (mut host, _closed_rx, _req_tx) = host_with_publishing_caller_and_child();
        let ctx = egui::Context::default();
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::Vec2::new(640.0, 480.0),
        ));

        let caller_ev_tx = host.ensure_occupant("CALLER").expect("occupant builds");
        host.show_occupant(Some("CALLER"));

        // Drive frames until CHILD spawns — CALLER's own `OpenFormSync` is a
        // real blocking round trip through the supervisor.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while host.children.is_empty() {
            let mut f = ctx.run_ui(input.clone(), |ui| host.ui_impl(ui));
            f.textures_delta.clear();
            assert!(std::time::Instant::now() < deadline, "CHILD never spawned");
            std::thread::sleep(std::time::Duration::from_millis(5));
        }

        // Drive frames until CHILD's SetProperty + Close both land and the
        // modal releases CALLER (bounded wait).
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !host.children.is_empty() {
            let mut f = ctx.run_ui(input.clone(), |ui| host.ui_impl(ui));
            f.textures_delta.clear();
            assert!(std::time::Instant::now() < deadline, "CHILD never closed");
            std::thread::sleep(std::time::Duration::from_millis(5));
        }

        // CALLER's own paragraph is now parked at its ONE `COBOL-WAIT-EVENT`
        // — inject a second event (any event on any control works; a real
        // subsequent UI action does exactly this) so it dispatches, drains
        // the queued property write into `self.objects`, and writes it to
        // Lbl-1::Caption, observable through the SAME `state` map the real
        // render engine reads (unlike DISPLAY, which `child_frame` drains
        // straight to stdout on this very same render pass).
        let _ = caller_ev_tx.send(FormEvent::new("Whatever", "onTick"));

        let mut caption = String::new();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            let mut f = ctx.run_ui(input.clone(), |ui| host.ui_impl(ui));
            f.textures_delta.clear();
            if let Some(occ) = host.pane.occupants.get("CALLER") {
                if let Some(cs) = occ.body.state.get("Lbl-1") {
                    if let Some((_, c)) =
                        cs.props.iter().find(|(k, _)| k.eq_ignore_ascii_case("Caption"))
                    {
                        caption = c.clone();
                        if caption.contains("ButtonOk clicked") {
                            break;
                        }
                    }
                }
            }
            assert!(
                std::time::Instant::now() < deadline,
                "CALLER's own me::\"GetProperty\" never saw what CHILD published \
                 through super::\"SetProperty\"; last seen Lbl-1::Caption: {caption:?}"
            );
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    /// 051 Q2 (operator ruling) — a PARKED form's enabled timers keep
    /// running: with an occupant on the pane, the root's Timer still fires
    /// onTick from the host's own clocks.
    #[test]
    fn parked_forms_keep_their_timers_ticking() {
        let form = cobolt_forms::Form::new("MAIN-FORM", "Main", 320, 200);
        let mut timer =
            cobolt_forms::Control::new("TMR-1", cobolt_forms::ControlType::Timer, 0, 0);
        timer.set_prop("Interval", cobolt_forms::model::PropValue::Int(25));
        timer.set_prop("Enabled", cobolt_forms::model::PropValue::Bool(true));
        let flat = vec![timer.clone()];
        let mut state = HashMap::new();
        state.insert("TMR-1".to_string(), CtrlState::from_control(&timer));

        let (ev_tx, ev_rx) = mpsc::channel();
        let (input_tx, _input_rx) = mpsc::channel();
        let (_state_tx, state_rx) = mpsc::channel();
        let (_display_tx, display_rx) = mpsc::channel();
        let (form_req_tx, form_req_rx) = mpsc::channel();
        let (closed_tx, _closed_rx) = mpsc::channel();
        let (mut host, _form) = FormHost::new(FormHostConfig {
            form,
            flat,
            state,
            ev_tx,
            input_tx,
            state_rx,
            display_rx,
            pending: Arc::new(AtomicUsize::new(0)),
            finished: Arc::new(AtomicBool::new(false)),
            form_req_rx,
            closed_tx,
            form_req_tx,
            form_source: None,
            child_theme: None,
            child_interpreter_setup: None,
            indexed_engine: Default::default(),
            shared_rust_bridge: None,
            fx_entrance: cobolt_forms::window_fx::FxSpec::default(),
            fx_exit: cobolt_forms::window_fx::FxSpec::default(),
            fx_restore: false,
            theme_pack: None,
            surface_theme: cobolt_forms::surface_theme::liquid_glass(),
            icon_path: None,
            title_fallback: String::new(),
            hooks: Box::new(NoHooks),
            surface: Surface::Pane,
        });

        // An occupant owns the pane, so the root is PARKED.
        host.pane.active_occupant = Some("SOMEONE-ELSE".into());
        let ctx = egui::Context::default();
        host.tick_parked_bodies(&ctx); // seeds the clock
        std::thread::sleep(std::time::Duration::from_millis(40));
        host.tick_parked_bodies(&ctx); // past the 25ms interval → fires
        let ticks: Vec<String> = ev_rx
            .try_iter()
            .filter(|e: &FormEvent| e.event_id == "onTick")
            .map(|e| e.ctrl_id)
            .collect();
        assert_eq!(
            ticks,
            vec!["TMR-1".to_string()],
            "the parked root's 25ms timer fired exactly once in ~40ms"
        );

        // On-pane again: the parked clocks reset so no stale burst follows.
        host.show_occupant(None);
        assert!(host.root.parked_timer_clocks.is_empty() || host.pane.active_occupant.is_none());

        println!(
            "parked timers — root parked behind an occupant: 1 onTick from a 25ms \
             timer across a 40ms park (coalesced, no burst)"
        );
    }

    /// 051 R15 — an open that cannot be satisfied is released visibly: the
    /// handle NULLs (NotifyClosed) and no dead window lingers.
    #[test]
    fn failed_spawn_is_released_never_silently_dropped() {
        // Host WITHOUT a form source (single-form runtime), and one WITH a
        // source but an unknown target: both fail the same honest way.
        for (with_source, label) in [(false, "no source"), (true, "unknown form")] {
            let (mut host, closed_rx, _req_tx) = host_with_source(with_source);
            let ctx = egui::Context::default();
            let mut input = egui::RawInput::default();
            input.screen_rect = Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::Vec2::new(640.0, 480.0),
            ));
            host.supervisor_open_for_test("GHOST");
            let mut full = ctx.run_ui(input, |ui| {
                host.apply_host_actions(ui.ctx(), vec![spawn_action("W1", "GHOST")]);
            });
            full.textures_delta.clear();
            assert!(host.children.is_empty(), "{label}: no dead window");
            let closed: Vec<String> = closed_rx.try_iter().collect();
            assert!(
                closed.contains(&"W1".to_string()),
                "{label}: the handle was released (NULLs at the caller): {closed:?}"
            );
        }
        println!("failed spawn — 2/2 failure modes release the handle visibly (R15)");
    }

    /// A root form blocked by a Sync child keeps ITS OWN theme. The child's
    /// window paints first in the frame and publishes the child's theme on
    /// the shared context; the root painted after it with whatever was left
    /// there, so a themed caller turned into its child's look until the
    /// child closed (operator report, Windows, 2026-09-29: "Cadastro de
    /// Empresas" lost its theme while "Mensagem do Sistema" was open).
    #[test]
    fn a_blocked_root_keeps_its_own_theme_under_a_differently_themed_child() {
        let mut form = cobolt_forms::Form::new("MAIN-FORM", "Main", 320, 200);
        form.controls.push(cobolt_forms::Control::new("Btn-1", cobolt_forms::ControlType::Button, 20, 20));
        let (ev_tx, _ev_rx) = mpsc::channel();
        let (input_tx, _input_rx) = mpsc::channel();
        let (_state_tx, state_rx) = mpsc::channel();
        let (_display_tx, display_rx) = mpsc::channel();
        let (form_req_tx, form_req_rx) = mpsc::channel();
        let (closed_tx, _closed_rx) = mpsc::channel();
        let source: Option<FormSource> = Some(Box::new(|id: &str| {
            if id.eq_ignore_ascii_case("CHILD") {
                Ok((
                    cobolt_forms::Form::new("CHILD", "Child", 240, 160),
                    program_from("IDENTIFICATION DIVISION.\nPROGRAM-ID. CHILD.\nPROCEDURE DIVISION.\n    STOP RUN.\n"),
                ))
            } else {
                Err(format!("no form named '{id}'"))
            }
        }));
        let (mut host, _form) = FormHost::new(FormHostConfig {
            flat: form.controls.clone(),
            form,
            state: HashMap::new(),
            ev_tx,
            input_tx,
            state_rx,
            display_rx,
            pending: Arc::new(AtomicUsize::new(0)),
            finished: Arc::new(AtomicBool::new(false)),
            form_req_rx,
            closed_tx,
            form_req_tx,
            form_source: source,
            // Children paint procedural Liquid Glass; the root is Elegance.
            child_theme: None,
            child_interpreter_setup: None,
            indexed_engine: Default::default(),
            shared_rust_bridge: None,
            fx_entrance: cobolt_forms::window_fx::FxSpec::default(),
            fx_exit: cobolt_forms::window_fx::FxSpec::default(),
            fx_restore: false,
            theme_pack: None,
            surface_theme: cobolt_forms::surface_theme::elegance(),
            icon_path: None,
            title_fallback: String::new(),
            hooks: Box::new(NoHooks),
            surface: Surface::Window,
        });
        let ctx = egui::Context::default();
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::new(640.0, 480.0)));
        let fills = |full: &egui::FullOutput| {
            fn walk(s: &egui::Shape, out: &mut Vec<egui::Color32>) {
                match s {
                    egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
                    egui::Shape::Rect(r) => out.push(r.fill),
                    _ => {}
                }
            }
            let mut out = Vec::new();
            for cs in &full.shapes {
                walk(&cs.shape, &mut out);
            }
            out
        };

        // The root, blocked by a Sync child that has no window yet: its own look.
        let (reply_tx, _reply_rx) = mpsc::channel();
        let _ = host.supervisor.handle_request(cobolt_runtime::form_host::FormRequest::OpenForm {
            caller: cobolt_runtime::form_host::ROOT_HANDLE.into(),
            form_id: "CHILD".into(),
            sync: true,
            window_state: None,
            x: None,
            y: None,
            width: None,
            height: None,
            modal: true,
            reply: reply_tx,
        });
        assert!(host.root_modal_blocked());
        let mut own = ctx.run_ui(input.clone(), |ui| host.ui_impl(ui));
        own.textures_delta.clear();
        let own = fills(&own);

        // The child's window opens, in Liquid Glass, and stays open.
        let mut f = ctx.run_ui(input.clone(), |ui| {
            host.apply_host_actions(ui.ctx(), vec![spawn_action("W1", "CHILD")]);
        });
        f.textures_delta.clear();
        let child = host.children.iter_mut().find(|c| c.handle == "W1").expect("child window exists");
        child.body.finished = Arc::new(AtomicBool::new(false));
        let mut with_child = ctx.run_ui(input.clone(), |ui| host.ui_impl(ui));
        with_child.textures_delta.clear();
        let with_child = fills(&with_child);

        let lost: Vec<_> = own.iter().filter(|c| !with_child.contains(c)).collect();
        assert!(lost.is_empty(), "the root lost its own fills while the child was open: {lost:?}");
        // …and wears the overlay its design chose over it.
        host.root.modal_overlay_style = cobolt_forms::model::ModalOverlayStyle::Greyed;
        let mut greyed = ctx.run_ui(input.clone(), |ui| host.ui_impl(ui));
        greyed.textures_delta.clear();
        assert!(
            fills(&greyed).contains(&egui::Color32::from_rgba_unmultiplied(60, 60, 64, 150)),
            "Greyed paints its layer over the root while the child's window is open"
        );
        println!("root under a Liquid Glass child: all {} of its Elegance fills kept", own.len());
    }

    /// Spec 083 — a Spatial form shown in the ContentPane of a shell whose own
    /// form is not Spatial still gets the desktop blurred behind it. The shell
    /// asked only its root form, so the embedded form showed the desktop sharp
    /// through the always-transparent shell window (operator, 2026-10-02).
    #[test]
    fn a_spatial_contentpane_occupant_blurs_the_shell() {
        let (mut host, _closed_rx, _req_tx) = host_with_caller_and_child();
        host.child_theme = Some(Box::new(|_form: &cobolt_forms::Form| {
            (None, cobolt_forms::surface_theme::spatial())
        }));
        assert!(!host.shell_wants_os_blur(), "a Liquid Glass shell alone does not blur");
        host.ensure_occupant("CALLER").expect("occupant builds");
        host.show_occupant(Some("CALLER"));
        assert!(host.shell_wants_os_blur(), "the Spatial occupant on the pane blurs it");
        host.show_occupant(None);
        assert!(!host.shell_wants_os_blur(), "and stops once the pane shows the root again");
    }
}

#[cfg(test)]
mod parity {
    use super::*;
    use cobolt_forms::window_fx::FxSpec;
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    struct Pipes {
        ev_rx: mpsc::Receiver<FormEvent>,
        _input_rx: mpsc::Receiver<StateUpdate>,
        _state_tx: mpsc::Sender<StateUpdate>,
        _display_tx: mpsc::Sender<String>,
        finished: Arc<AtomicBool>,
        _form_req_tx: mpsc::Sender<cobolt_runtime::form_host::FormRequest>,
        _closed_rx: mpsc::Receiver<String>,
    }

    /// A form with a `Transparency` of its own gets a see-through window, as
    /// an entrance over the desktop always did; a pane never does, and an
    /// opaque form stays opaque (property audit, 2026-09-26).
    #[test]
    fn a_transparent_form_gets_a_see_through_window() {
        let build = |transparency: u8, surface: Surface| -> bool {
            let mut form = cobolt_forms::Form::new("SEE", "See", 320, 200);
            form.transparency = transparency;
            let (ev_tx, _ev_rx) = mpsc::channel();
            let (input_tx, _input_rx) = mpsc::channel();
            let (_state_tx, state_rx) = mpsc::channel();
            let (_display_tx, display_rx) = mpsc::channel();
            let (form_req_tx, form_req_rx) = mpsc::channel();
            let (closed_tx, _closed_rx) = mpsc::channel();
            let (host, _f) = FormHost::new(FormHostConfig {
                form,
                flat: Vec::new(),
                state: HashMap::new(),
                ev_tx,
                input_tx,
                state_rx,
                display_rx,
                pending: Arc::new(AtomicUsize::new(0)),
                finished: Arc::new(AtomicBool::new(false)),
                form_req_rx,
                closed_tx,
                form_req_tx,
                form_source: None,
                child_theme: None,
                child_interpreter_setup: None,
                indexed_engine: Default::default(),
                shared_rust_bridge: None,
                fx_entrance: FxSpec::default(),
                fx_exit: FxSpec::default(),
                fx_restore: false,
                theme_pack: None,
                surface_theme: cobolt_forms::surface_theme::liquid_glass(),
                icon_path: None,
                title_fallback: String::new(),
                hooks: Box::new(NoHooks),
                surface,
            });
            host.see_through
        };
        assert!(build(30, Surface::Window));
        assert!(!build(0, Surface::Window));
        assert!(!build(30, Surface::Pane));
    }

    /// Form `CornerRadius` (operator, 2026-10-03): a form designed with a
    /// radius gets a see-through window — alpha can only be chosen when the
    /// window is created — title bar or not, so it can round whenever the
    /// title bar goes. Its backdrop is cut to the window's arc only while the
    /// title bar is off, and `me::CornerRadius` changes the radius it uses.
    #[test]
    fn a_rounded_form_gets_a_see_through_window_rounded_without_a_title_bar() {
        let build = |radius: u32, title: bool| -> FormHost {
            let mut form = cobolt_forms::Form::new("RND", "Round", 320, 200);
            form.corner_radius = radius;
            form.title_visible = title;
            let (ev_tx, _ev_rx) = mpsc::channel();
            let (input_tx, _input_rx) = mpsc::channel();
            let (_state_tx, state_rx) = mpsc::channel();
            let (_display_tx, display_rx) = mpsc::channel();
            let (form_req_tx, form_req_rx) = mpsc::channel();
            let (closed_tx, _closed_rx) = mpsc::channel();
            FormHost::new(FormHostConfig {
                form,
                flat: Vec::new(),
                state: HashMap::new(),
                ev_tx,
                input_tx,
                state_rx,
                display_rx,
                pending: Arc::new(AtomicUsize::new(0)),
                finished: Arc::new(AtomicBool::new(false)),
                form_req_rx,
                closed_tx,
                form_req_tx,
                form_source: None,
                child_theme: None,
                child_interpreter_setup: None,
                indexed_engine: Default::default(),
                shared_rust_bridge: None,
                fx_entrance: FxSpec::default(),
                fx_exit: FxSpec::default(),
                fx_restore: false,
                theme_pack: None,
                surface_theme: cobolt_forms::surface_theme::liquid_glass(),
                icon_path: None,
                title_fallback: String::new(),
                hooks: Box::new(NoHooks),
                surface: Surface::Window,
            })
            .0
        };
        let ctx = egui::Context::default();
        ctx.run_ui(egui::RawInput::default(), |_| {}).textures_delta.clear();
        let arc = |h: &FormHost| h.root.backdrop(&ctx, egui::vec2(320.0, 200.0)).window;

        let square = build(0, false);
        assert!(!square.see_through, "a square form keeps an opaque window");
        assert_eq!(arc(&square), None);

        let titled = build(20, true);
        assert!(titled.see_through, "designed with a radius: created see-through, title bar or not");
        assert_eq!(arc(&titled), None, "a titled window's corners are the OS's");

        let mut frameless = build(20, false);
        assert!(frameless.see_through);
        let (face, rad, flags) = arc(&frameless).expect("rounded without a title bar");
        assert_eq!((rad, flags), (20.0, [true; 4]));
        assert_eq!(face, ctx.content_rect(), "the arc is the window's own");

        let written = frameless.root.apply_form_window_update(
            &ctx,
            &StateUpdate::new("RND", "CornerRadius", "36"),
            None,
        );
        assert!(written, "me::CornerRadius is a form window property");
        assert_eq!(arc(&frameless).map(|(_, r, _)| r), Some(36.0));
        println!("CornerRadius: 0 → opaque, square; 20 titled → see-through, square; 20 frameless → rounded 20; me::CornerRadius 36 → 36");
    }

    /// Spec 083 R2/R3 (AC2): a Spatial form gets a see-through window and
    /// asks for the desktop to be blurred, with the theme's translucent glass
    /// as its backdrop even when the form carries a solid colour; a Liquid
    /// Glass form does neither.
    #[test]
    fn a_spatial_form_is_see_through_and_blurred() {
        let build = |theme: std::sync::Arc<dyn cobolt_forms::surface_theme::SurfaceTheme>| -> FormHost {
            let mut form = cobolt_forms::Form::new("SPACE", "Space", 320, 200);
            form.background_color = "#F4F6F9FF".into();
            let (ev_tx, _ev_rx) = mpsc::channel();
            let (input_tx, _input_rx) = mpsc::channel();
            let (_state_tx, state_rx) = mpsc::channel();
            let (_display_tx, display_rx) = mpsc::channel();
            let (form_req_tx, form_req_rx) = mpsc::channel();
            let (closed_tx, _closed_rx) = mpsc::channel();
            let (host, _f) = FormHost::new(FormHostConfig {
                form,
                flat: Vec::new(),
                state: HashMap::new(),
                ev_tx,
                input_tx,
                state_rx,
                display_rx,
                pending: Arc::new(AtomicUsize::new(0)),
                finished: Arc::new(AtomicBool::new(false)),
                form_req_rx,
                closed_tx,
                form_req_tx,
                form_source: None,
                child_theme: None,
                child_interpreter_setup: None,
                indexed_engine: Default::default(),
                shared_rust_bridge: None,
                fx_entrance: FxSpec::default(),
                fx_exit: FxSpec::default(),
                fx_restore: false,
                theme_pack: None,
                surface_theme: theme,
                icon_path: None,
                title_fallback: String::new(),
                hooks: Box::new(NoHooks),
                surface: Surface::Window,
            });
            host
        };
        let spatial = build(cobolt_forms::surface_theme::spatial());
        assert!(spatial.see_through, "a Spatial window is see-through");
        assert!(spatial.wants_os_blur(), "and asks for the desktop blur");
        let ctx = egui::Context::default();
        let backdrop = spatial.root.backdrop(&ctx, egui::vec2(320.0, 200.0));
        let alpha = u8::from_str_radix(&backdrop.color_hex[7..9], 16).unwrap();
        assert!(alpha < 255, "the backdrop is the theme's translucent glass, not #F4F6F9FF: {}", backdrop.color_hex);
        let glass = build(cobolt_forms::surface_theme::liquid_glass());
        assert!(!glass.see_through && !glass.wants_os_blur(), "Liquid Glass keeps its window");
        assert_eq!(glass.root.backdrop(&ctx, egui::vec2(320.0, 200.0)).color_hex, "#F4F6F9FF");
    }

    /// Property audit, 2026-09-26: a COBOL write to the form's own window
    /// properties reaches the running form — BackgroundColor / Transparency /
    /// Width / Height update the body, and Title / Width become commands to
    /// the window (none in the shell's pane, which does not own it).
    #[test]
    fn a_form_property_write_reaches_the_window() {
        let (mut host, _pipes) = host_with("none:600:ease-out", "none:600:ease-out", false);
        let ctx = egui::Context::default();
        let form = host.root.form_object.clone();
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            let ctx = ui.ctx().clone();
            for (k, v) in [
                ("BackgroundColor", "#112233FF"),
                ("Transparency", "40"),
                ("Title", "Processing..."),
                ("Width", "640"),
            ] {
                assert!(host.root.apply_form_window_update(&ctx, &StateUpdate::new(form.clone(), k, v), Some(egui::ViewportId::ROOT)));
            }
            // Someone else's property is not the form's.
            assert!(!host.root.apply_form_window_update(&ctx, &StateUpdate::new("BTN-1", "Title", "x"), Some(egui::ViewportId::ROOT)));
        });
        out.textures_delta.clear();
        assert_eq!(host.root.bg_hex, "#112233FF");
        assert_eq!(host.root.transparency, 40);
        assert_eq!(host.root.form_size.x, 640.0);
        let cmds = &out.viewport_output[&egui::ViewportId::ROOT].commands;
        assert!(cmds.iter().any(|c| matches!(c, egui::ViewportCommand::Title(t) if t == "Processing...")), "{cmds:?}");
        assert!(cmds.iter().any(|c| matches!(c, egui::ViewportCommand::InnerSize(_))), "{cmds:?}");

        // In a pane the body still takes the value, and no window command goes out.
        let (mut pane, _p) = host_with_surface("none:600:ease-out", "none:600:ease-out", false, Surface::Pane);
        let form = pane.root.form_object.clone();
        let out = ctx.run_ui(egui::RawInput::default(), |ui| {
            let ctx = ui.ctx().clone();
            pane.root.apply_form_window_update(&ctx, &StateUpdate::new(form.clone(), "Title", "x"), None);
            pane.root.apply_form_window_update(&ctx, &StateUpdate::new(form.clone(), "BackgroundColor", "#445566FF"), None);
        });
        assert_eq!(pane.root.bg_hex, "#445566FF");
        let cmds = out.viewport_output.get(&egui::ViewportId::ROOT).map(|v| v.commands.clone()).unwrap_or_default();
        assert!(!cmds.iter().any(|c| matches!(c, egui::ViewportCommand::Title(_))), "the pane sends no title");
    }

    /// Ctrl+W (Cmd+W on macOS) asks the main window to close, the same request
    /// its close button makes (operator, 2026-10-03); a host drawn in a shell's
    /// pane does not own the window and leaves the key to the shell.
    #[test]
    fn command_w_asks_the_main_window_to_close() {
        let closes = |surface: Surface| -> bool {
            let (mut host, _pipes) = host_with_surface("none:600:ease-out", "none:600:ease-out", false, surface);
            let ctx = egui::Context::default();
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::new(640.0, 480.0))),
                events: vec![egui::Event::ModifiersChanged(egui::Modifiers::COMMAND), egui::Event::Key {
                    key: egui::Key::W,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::COMMAND,
                }],
                ..Default::default()
            };
            let mut out = ctx.run_ui(input, |ui| host.ui_impl(ui));
            out.textures_delta.clear();
            out.viewport_output
                .get(&egui::ViewportId::ROOT)
                .is_some_and(|v| v.commands.iter().any(|c| matches!(c, egui::ViewportCommand::Close)))
        };
        assert!(closes(Surface::Window), "Cmd/Ctrl+W asks the window to close");
        assert!(!closes(Surface::Pane), "a pane leaves the key to the shell that owns the window");
    }

    fn host_with(entrance: &str, exit: &str, restore: bool) -> (FormHost, Pipes) {
        host_with_surface(entrance, exit, restore, Surface::Window)
    }

    /// **`SET x::VISIBLE TO 1` must undo `SET x::VISIBLE TO 0`.** Reported as
    /// hiding working and showing not (operator, 2026-08-20). Drives the exact
    /// pair through the same entry point a running interpreter uses, and reads
    /// back what the renderer's visibility gate reads.
    #[test]
    fn showing_a_control_again_undoes_hiding_it() {
        use cobolt_forms::render::FormState;
        let form = cobolt_forms::Form::new("MAIN", "Main", 320, 200);
        let mut sw = cobolt_forms::Control::new("Switch-1", cobolt_forms::ControlType::Switch, 10, 10);
        sw.rect = cobolt_forms::model::Rect::new(10, 10, 60, 24);
        let flat = vec![sw.clone()];
        let (ev_tx, _ev_rx) = mpsc::channel();
        let (input_tx, _input_rx) = mpsc::channel();
        let (_state_tx, state_rx) = mpsc::channel();
        let (_display_tx, display_rx) = mpsc::channel();
        let (_form_req_tx, form_req_rx) = mpsc::channel();
        let (closed_tx, _closed_rx) = mpsc::channel();
        let (mut host, _f) = FormHost::new(FormHostConfig {
            form,
            flat,
            state: HashMap::new(),
            ev_tx,
            input_tx,
            state_rx,
            display_rx,
            pending: Arc::new(AtomicUsize::new(0)),
            finished: Arc::new(AtomicBool::new(false)),
            form_req_rx,
            closed_tx,
            form_req_tx: _form_req_tx.clone(),
            form_source: None,
            child_theme: None,
            child_interpreter_setup: None,
            indexed_engine: Default::default(),
            shared_rust_bridge: None,
            fx_entrance: FxSpec::default(),
            fx_exit: FxSpec::default(),
            fx_restore: false,
            theme_pack: None,
            surface_theme: cobolt_forms::surface_theme::liquid_glass(),
            icon_path: None,
            title_fallback: String::new(),
            hooks: Box::new(NoHooks),
            surface: Surface::Window,
        });

        // What the renderer asks, through the very same `FormState` it uses.
        let is_visible = |h: &FormHost| -> bool {
            let st = crate::state::LiveState {
                state: &h.root.state,
                anim: &h.root.anim,
                hidden: None,
                viewer_docs: None,
                special_names: &h.root.special_names,
            };
            st.visible(&sw)
        };

        assert!(is_visible(&host), "a designed-visible Switch starts visible");

        // COBOL upper-cases unquoted identifiers, so this is the shape the
        // interpreter actually sends.
        host.root
            .apply_interpreter_update(StateUpdate::new("SWITCH-1", "VISIBLE", "0"), false);
        assert!(!is_visible(&host), "SET …::VISIBLE TO 0 hides it");

        host.root
            .apply_interpreter_update(StateUpdate::new("SWITCH-1", "VISIBLE", "1"), false);
        assert!(
            is_visible(&host),
            "SET …::VISIBLE TO 1 must bring it back — hiding is not one-way"
        );
    }

    /// **Clicking a tab changes the page in a RUN form, not only in Preview.**
    ///
    /// `containers::is_visible` decides which page's controls are drawn from the
    /// `ActiveTabs` map, falling back to the DESIGNED `SelectedTab` when the map
    /// has no entry. Every host frame built that map as
    /// `ActiveTabs::default()` — empty, and rebuilt empty each frame — so the
    /// fallback was the only thing that ever answered and the designed page was
    /// the only page a running form could ever show. The click was not lost:
    /// it wrote `SelectedTab` into the live state and was forwarded to the
    /// interpreter. Nothing read it back (operator, 2026-09-09: "TabControl:
    /// Clicking in a Run form does not change the page. Preview works fine").
    ///
    /// Driven exactly as a click drives it: `forward_interaction` writes the
    /// prop update into the live state, which is what this asserts against.
    #[test]
    fn a_tab_click_changes_the_page_in_a_run_form() {
        let mut form = cobolt_forms::Form::new("TAB-FORM", "Tabs", 400, 300);
        let mut tabs =
            cobolt_forms::Control::new("TABS-1", cobolt_forms::ControlType::TabControl, 10, 10);
        tabs.rect = cobolt_forms::model::Rect::new(10, 10, 360, 240);
        tabs.set_prop("Tabs", cobolt_forms::PropValue::String("One\nTwo".into()));
        tabs.set_prop("SelectedTab", cobolt_forms::PropValue::Int(1));
        let mut page0 =
            cobolt_forms::Control::new("LBL-ONE", cobolt_forms::ControlType::Label, 30, 60);
        page0.parent = Some("TABS-1".into());
        page0.tab = Some(1);
        let mut page1 =
            cobolt_forms::Control::new("LBL-TWO", cobolt_forms::ControlType::Label, 30, 60);
        page1.parent = Some("TABS-1".into());
        page1.tab = Some(2);
        form.controls.push(tabs.clone());
        let flat = vec![tabs, page0, page1];

        let (ev_tx, _ev_rx) = mpsc::channel();
        let (input_tx, _input_rx) = mpsc::channel();
        let (_state_tx, state_rx) = mpsc::channel();
        let (_display_tx, display_rx) = mpsc::channel();
        let (_form_req_tx, form_req_rx) = mpsc::channel();
        let (closed_tx, _closed_rx) = mpsc::channel();
        let (mut host, _f) = FormHost::new(FormHostConfig {
            form,
            flat: flat.clone(),
            state: HashMap::new(),
            ev_tx,
            input_tx,
            state_rx,
            display_rx,
            pending: Arc::new(AtomicUsize::new(0)),
            finished: Arc::new(AtomicBool::new(false)),
            form_req_rx,
            closed_tx,
            form_req_tx: _form_req_tx.clone(),
            form_source: None,
            child_theme: None,
            child_interpreter_setup: None,
            indexed_engine: Default::default(),
            shared_rust_bridge: None,
            fx_entrance: FxSpec::default(),
            fx_exit: FxSpec::default(),
            fx_restore: false,
            theme_pack: None,
            surface_theme: cobolt_forms::surface_theme::liquid_glass(),
            icon_path: None,
            title_fallback: String::new(),
            hooks: Box::new(NoHooks),
            surface: Surface::Window,
        });

        // Exactly the question the renderer asks, through the map the host
        // hands it.
        let shows = |h: &FormHost, id: &str| -> bool {
            let controls = &h.root.controls;
            let idx = controls
                .iter()
                .position(|c| c.id == id)
                .expect("the control is in the form");
            let st = crate::state::LiveState {
                state: &h.root.state,
                anim: &h.root.anim,
                hidden: None,
                viewer_docs: None,
                special_names: &h.root.special_names,
            };
            cobolt_forms::containers::is_visible(controls, idx, &h.root.active_tabs(), &|c| {
                cobolt_forms::render::FormState::visible(&st, c)
            })
        };

        assert!(shows(&host, "LBL-ONE"), "page 1 is the designed page");
        assert!(!shows(&host, "LBL-TWO"), "page 2 starts hidden");

        // What a click does: `forward_interaction` writes the prop update into
        // the live state under the DESIGNED spelling.
        host.root.state_entry_mut("TABS-1").set("SelectedTab", "2".into());
        assert!(
            shows(&host, "LBL-TWO"),
            "clicking tab 2 must show page 2 — this is the operator's report"
        );
        assert!(!shows(&host, "LBL-ONE"), "…and hide page 1");

        // And through the interpreter's own spelling, which arrives upper-cased.
        host.root
            .apply_interpreter_update(StateUpdate::new("TABS-1", "SELECTEDTAB", "1"), false);
        assert!(
            shows(&host, "LBL-ONE"),
            "SET TABS-1::SelectedTab TO 1 must go back to page 1"
        );
        assert!(!shows(&host, "LBL-TWO"), "…and hide page 2 again");
    }

    /// **A main form's Viewer gets its Save panel answered.** Only the child
    /// path opened and collected it, so on the ROOT form — PowerChat's chat,
    /// the first form of every built application — `SaveAsPdf()` and
    /// `SaveAs()` did nothing at all (operator, 2026-09-28: "Save as PDF was
    /// not implemented"). The OS dialog is stood in for by its test hook; what
    /// is proved is that the root frame collects the answer and hands it to
    /// the program.
    #[test]
    fn a_main_forms_viewer_gets_its_save_panel_answered() {
        let mut form = cobolt_forms::Form::new("MAIN", "Main", 400, 300);
        let mut v = cobolt_forms::Control::new("VWR-1", cobolt_forms::ControlType::Viewer, 10, 10);
        v.rect = cobolt_forms::model::Rect::new(10, 10, 300, 200);
        form.add_control(v);
        let mut flat = Vec::new();
        crate::flatten_controls(&form.controls, &mut flat);
        let state: HashMap<String, CtrlState> =
            flat.iter().map(|c| (c.id.clone(), CtrlState::from_control(c))).collect();
        let (ev_tx, ev_rx) = mpsc::channel::<FormEvent>();
        let (input_tx, input_rx) = mpsc::channel::<StateUpdate>();
        let (_state_tx, state_rx) = mpsc::channel();
        let (_display_tx, display_rx) = mpsc::channel();
        let (form_req_tx, form_req_rx) = mpsc::channel();
        let (closed_tx, _closed_rx) = mpsc::channel();
        let (mut app, _f) = FormHost::new(FormHostConfig {
            form, flat, state, ev_tx, input_tx, state_rx, display_rx,
            pending: Arc::new(AtomicUsize::new(0)),
            finished: Arc::new(AtomicBool::new(false)),
            form_req_rx, closed_tx, form_req_tx,
            form_source: None, child_theme: None, child_interpreter_setup: None, indexed_engine: Default::default(),
            shared_rust_bridge: None,
            fx_entrance: FxSpec::default(), fx_exit: FxSpec::default(), fx_restore: false,
            theme_pack: None,
            surface_theme: cobolt_forms::surface_theme::liquid_glass(),
            icon_path: None, title_fallback: String::new(), hooks: Box::new(NoHooks),
            surface: Surface::Window,
        });
        app.fx_entrance_done = true;
        app.root.anim_started = true;
        app.root.lifecycle_sent = true;
        // The operator chose where to save, in the panel the request opened.
        let chosen = std::env::temp_dir().join("conversation.pdf");
        crate::file_dialog::answer_for_test(&FormBody::viewer_save_as_key("VWR-1"), Some(chosen.clone()));
        let ctx = egui::Context::default();
        frame(&mut app, &ctx, raw());
        let answers: Vec<String> = input_rx
            .try_iter()
            .filter(|u| u.ctrl_id == "VWR-1" && u.prop == "_SaveAsAnswer")
            .map(|u| u.value)
            .collect();
        println!("root form: the Save panel's answer reached the program: {answers:?}");
        assert_eq!(answers, vec![chosen.display().to_string()], "the main form collects its Viewer's Save panel");
        // …and wakes the program to read it: an idle program waits on events,
        // so the answer — and the PDF it writes — used to wait for the next
        // unrelated one (operator, 2026-09-29: "it took a long time").
        let woken = ev_rx
            .try_iter()
            .any(|e| e.ctrl_id == "VWR-1" && e.event_id == cobolt_runtime::form_host::FormSupervisor::INPUT_WAKE_EVENT);
        assert!(woken, "the program is woken to read the answer");
    }

    fn host_with_surface(
        entrance: &str,
        exit: &str,
        restore: bool,
        surface: Surface,
    ) -> (FormHost, Pipes) {
        let form = cobolt_forms::Form::new("PARITY-FORM", "Parity", 320, 200);
        let (ev_tx, ev_rx) = mpsc::channel();
        let (input_tx, _input_rx) = mpsc::channel();
        let (_state_tx, state_rx) = mpsc::channel();
        let (_display_tx, display_rx) = mpsc::channel();
        let (_form_req_tx, form_req_rx) = mpsc::channel();
        let (closed_tx, _closed_rx) = mpsc::channel();
        let pending = Arc::new(AtomicUsize::new(0));
        let finished = Arc::new(AtomicBool::new(false));
        let (host, _form) = FormHost::new(FormHostConfig {
            form,
            flat: Vec::new(),
            state: HashMap::new(),
            ev_tx,
            input_tx,
            state_rx,
            display_rx,
            pending,
            finished: Arc::clone(&finished),
            form_req_rx,
            closed_tx,
            form_req_tx: _form_req_tx.clone(),
            form_source: None,
            child_theme: None,
            child_interpreter_setup: None,
            indexed_engine: Default::default(),
            shared_rust_bridge: None,
            fx_entrance: FxSpec::parse(entrance),
            fx_exit: FxSpec::parse(exit),
            fx_restore: restore,
            theme_pack: None,
            surface_theme: cobolt_forms::surface_theme::liquid_glass(),
            icon_path: None,
            title_fallback: String::new(),
            hooks: Box::new(NoHooks),
            surface,
        });
        (
            host,
            Pipes {
                ev_rx,
                _input_rx,
                _state_tx,
                _display_tx,
                finished,
                _form_req_tx,
                _closed_rx,
            },
        )
    }

    fn raw() -> egui::RawInput {
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(800.0, 600.0),
            )),
            ..Default::default()
        }
    }

    /// The backdrop is laid out against the SURFACE the body is drawn into,
    /// not against the window.
    ///
    /// `backdrop` used to read `ctx.content_rect()` itself, so a form loaded
    /// into the shell's ContentPane had its gradient and background image
    /// sized and centred on the whole window — a rectangle wider than the pane
    /// by the MenuPane rail and taller by the breadcrumb band. Fit letterboxed
    /// outside the visible pane, Center centred on the window's midpoint, and
    /// the picture slid whenever the window resized while the pane did not
    /// move with it (operator, 2026-08-31).
    ///
    /// `window_size` IS the extent the engine evaluates every mode against
    /// (`render::backdrop_size`), so asserting it here is asserting the modes.
    #[test]
    fn the_backdrop_extent_is_the_surface_not_the_window() {
        let (host, _pipes) = host_with_surface("", "", false, Surface::Pane);
        let ctx = egui::Context::default();
        let mut warm = ctx.run_ui(raw(), |_| {});
        warm.textures_delta.clear();
        let window = ctx.content_rect().size();

        // A window form: the surface IS the window, and nothing changes.
        let as_window = host.root.backdrop(&ctx, window);
        assert_eq!(
            as_window.window_size,
            Some(window),
            "a form that owns its window is laid out against the window"
        );

        // The ContentPane occupant: the shell hands it the pane, and that is
        // what must reach the engine — never `ctx.content_rect()`.
        let pane = egui::vec2(window.x - 200.0, window.y - 48.0);
        let as_pane = host.root.backdrop(&ctx, pane);
        assert_eq!(
            as_pane.window_size,
            Some(pane),
            "an occupant is laid out against the PANE it is drawn into"
        );
        assert_ne!(
            as_pane.window_size, as_window.window_size,
            "the two must not collapse back together — that was the defect"
        );

        println!(
            "  backdrop extent — window {:.0}x{:.0} · pane {:.0}x{:.0}: each mode \
             is evaluated against its own surface",
            window.x, window.y, pane.x, pane.y
        );
    }

    /// 049 — in a pane the SideMenu is painted by the shell as the MenuPane,
    /// so the hosted form must not paint it a second time inside the
    /// ContentPane. In a window nothing changes.
    #[test]
    fn a_pane_host_does_not_paint_the_sidebar_twice_049() {
        fn host_with_controls(surface: Surface) -> FormHost {
            let form = cobolt_forms::Form::new("MAIN", "Main", 320, 200);
            let flat = vec![
                cobolt_forms::Control::new("SIDE-1", cobolt_forms::ControlType::SideMenu, 0, 0),
                cobolt_forms::Control::new("BTN-1", cobolt_forms::ControlType::Button, 10, 10),
                cobolt_forms::Control::new("BAR-1", cobolt_forms::ControlType::MenuBar, 0, 0),
            ];
            let (ev_tx, _ev_rx) = mpsc::channel();
            let (input_tx, _input_rx) = mpsc::channel();
            let (_state_tx, state_rx) = mpsc::channel();
            let (_display_tx, display_rx) = mpsc::channel();
            let (_form_req_tx, form_req_rx) = mpsc::channel();
            let (closed_tx, _closed_rx) = mpsc::channel();
            let (host, _form) = FormHost::new(FormHostConfig {
                form,
                flat,
                state: HashMap::new(),
                ev_tx,
                input_tx,
                state_rx,
                display_rx,
                pending: Arc::new(AtomicUsize::new(0)),
                finished: Arc::new(AtomicBool::new(false)),
                form_req_rx,
                closed_tx,
                form_req_tx: _form_req_tx.clone(),
                form_source: None,
                child_theme: None,
                child_interpreter_setup: None,
                indexed_engine: Default::default(),
                shared_rust_bridge: None,
                fx_entrance: FxSpec::default(),
                fx_exit: FxSpec::default(),
                fx_restore: false,
                theme_pack: None,
                surface_theme: cobolt_forms::surface_theme::liquid_glass(),
                icon_path: None,
                title_fallback: String::new(),
                hooks: Box::new(NoHooks),
                surface,
            });
            host
        }

        let ids = |h: &FormHost| -> Vec<String> {
            h.root.controls.iter().map(|c| c.id.clone()).collect()
        };

        let window = ids(&host_with_controls(Surface::Window));
        assert_eq!(
            window,
            vec!["SIDE-1", "BTN-1", "BAR-1"],
            "a window host renders every designed control, unchanged"
        );

        let pane = ids(&host_with_controls(Surface::Pane));
        assert_eq!(
            pane,
            vec!["BTN-1", "BAR-1"],
            "a pane host drops ONLY the SideMenu — the shell already paints it"
        );

        println!(
            "049 pane sidebar — window: 3/3 controls painted (SideMenu, Button, \
             MenuBar); pane: 2/3, the SideMenu alone withheld (a MenuBar still \
             paints, it is not shell chrome)"
        );
    }

    /// 049 — the ContentPane is JUXTAPOSED to the rail, not offset from it
    /// twice.
    ///
    /// The shell lays the pane out beside the MenuPane, so the pane's own left
    /// edge is already past the rail. A control still carrying its designed x
    /// was then pushed right by the rail's width a second time: a button drawn
    /// beside a 200pt rail landed 200pt into the pane, i.e. 400pt from the
    /// window edge. The operator photographed exactly that.
    #[test]
    fn a_pane_host_slides_the_form_over_the_rails_column_049() {
        fn host_for(surface: Surface) -> FormHost {
            let form = cobolt_forms::Form::new("MAIN", "Main", 960, 744);
            let mut side =
                cobolt_forms::Control::new("SIDE-1", cobolt_forms::ControlType::SideMenu, 0, 0);
            side.rect = cobolt_forms::model::Rect::new(0, 0, 200, 744);
            let mut beside =
                cobolt_forms::Control::new("BTN-1", cobolt_forms::ControlType::Button, 210, 40);
            beside.rect = cobolt_forms::model::Rect::new(210, 40, 100, 30);
            // Parked UNDER the rail: it must clamp to the pane's edge rather
            // than sliding off the left and out of reach.
            let mut under =
                cobolt_forms::Control::new("BTN-2", cobolt_forms::ControlType::Button, 20, 300);
            under.rect = cobolt_forms::model::Rect::new(20, 300, 100, 30);
            let (ev_tx, _ev_rx) = mpsc::channel();
            let (input_tx, _input_rx) = mpsc::channel();
            let (_state_tx, state_rx) = mpsc::channel();
            let (_display_tx, display_rx) = mpsc::channel();
            let (_form_req_tx, form_req_rx) = mpsc::channel();
            let (closed_tx, _closed_rx) = mpsc::channel();
            let (host, _form) = FormHost::new(FormHostConfig {
                form,
                flat: vec![side, beside, under],
                state: HashMap::new(),
                ev_tx,
                input_tx,
                state_rx,
                display_rx,
                pending: Arc::new(AtomicUsize::new(0)),
                finished: Arc::new(AtomicBool::new(false)),
                form_req_rx,
                closed_tx,
                form_req_tx: _form_req_tx.clone(),
                form_source: None,
                child_theme: None,
                child_interpreter_setup: None,
                indexed_engine: Default::default(),
                shared_rust_bridge: None,
                fx_entrance: FxSpec::default(),
                fx_exit: FxSpec::default(),
                fx_restore: false,
                theme_pack: None,
                surface_theme: cobolt_forms::surface_theme::liquid_glass(),
                icon_path: None,
                title_fallback: String::new(),
                hooks: Box::new(NoHooks),
                surface,
            });
            host
        }

        let x_of = |h: &FormHost, id: &str| -> i32 {
            h.root.controls.iter().find(|c| c.id == id).expect("control").rect.x
        };

        // A window host is untouched: the rail is a control there like any
        // other, and nothing about existing forms may move.
        let window = host_for(Surface::Window);
        assert_eq!(x_of(&window, "BTN-1"), 210, "a window host keeps designed x");
        assert_eq!(x_of(&window, "BTN-2"), 20);
        assert_eq!(window.designed_size().x, 960.0, "and the whole designed width");

        let pane = host_for(Surface::Pane);
        assert_eq!(
            x_of(&pane, "BTN-1"),
            10,
            "beside a 200pt rail ⇒ 10pt into the pane, not 210"
        );
        assert_eq!(
            x_of(&pane, "BTN-2"),
            0,
            "a control under the rail clamps to the pane edge, never off it"
        );
        assert_eq!(
            pane.designed_size().x,
            760.0,
            "the pane holds the form minus the rail's column (960 - 200)"
        );
        assert_eq!(
            pane.designed_size().y,
            744.0,
            "height is untouched: the breadcrumb is chrome outside the pane, \
             and shrinking here would cut the bottom of the form off"
        );

        println!(
            "049 pane juxtaposition — 960px form, 200px rail: BTN-1 210→10, \
             BTN-2 20→0 (clamped), pane content width 960→760; a window host \
             unchanged at 210/20/960"
        );
    }

    /// **The SideMenu's footer Panel belongs to the RAIL, not to the content.**
    ///
    /// Operator, 2026-08-22: a clock designed into the footer sat in the footer
    /// in the RAD and surfaced beside the rail, over the content, when the form
    /// ran. The panel sits at `x = 0` INSIDE the rail's column, so the pane's
    /// "slide the form over the rail" step drove it to `0 - rail`, clamped it
    /// at the pane's left edge, and drew it there — with its children.
    ///
    /// Two things are asserted, because either alone would hide the bug: the
    /// subtree keeps its DESIGNED rect (it is not content and must not slide),
    /// and the content pass WITHHOLDS it (the rail's own pass draws it, and
    /// drawing it twice would be its own bug).
    #[test]
    fn a_side_menu_footer_panel_is_the_rails_business_not_the_panes_049() {
        use cobolt_forms::render::FormState;

        fn host_for(surface: Surface) -> FormHost {
            let form = cobolt_forms::Form::new("MAIN", "Main", 960, 744);
            let mut side =
                cobolt_forms::Control::new("SIDE-1", cobolt_forms::ControlType::SideMenu, 0, 0);
            side.rect = cobolt_forms::model::Rect::new(0, 0, 200, 744);
            // The footer Panel the SideMenu owns, pinned to the bottom of the
            // rail's column exactly as `sync_side_menu_footer_panels` pins it.
            let mut footer =
                cobolt_forms::Control::new("SIDE-1-Footer", cobolt_forms::ControlType::Panel, 0, 600);
            footer.rect = cobolt_forms::model::Rect::new(0, 600, 200, 144);
            footer.parent = Some("SIDE-1".into());
            footer.set_prop(cobolt_forms::model::SIDE_MENU_FOOTER_PROP, true);
            // The operator's clock, dropped into that Panel.
            let mut clock =
                cobolt_forms::Control::new("LBL-CLOCK", cobolt_forms::ControlType::Label, 20, 640);
            clock.rect = cobolt_forms::model::Rect::new(20, 640, 160, 40);
            clock.parent = Some("SIDE-1-Footer".into());
            // Ordinary content, to prove the slide still happens for everything
            // that is NOT the footer.
            let mut beside =
                cobolt_forms::Control::new("BTN-1", cobolt_forms::ControlType::Button, 210, 40);
            beside.rect = cobolt_forms::model::Rect::new(210, 40, 100, 30);
            let (ev_tx, _ev_rx) = mpsc::channel();
            let (input_tx, _input_rx) = mpsc::channel();
            let (_state_tx, state_rx) = mpsc::channel();
            let (_display_tx, display_rx) = mpsc::channel();
            let (_form_req_tx, form_req_rx) = mpsc::channel();
            let (closed_tx, _closed_rx) = mpsc::channel();
            let (host, _form) = FormHost::new(FormHostConfig {
                form,
                flat: vec![side, footer, clock, beside],
                state: HashMap::new(),
                ev_tx,
                input_tx,
                state_rx,
                display_rx,
                pending: Arc::new(AtomicUsize::new(0)),
                finished: Arc::new(AtomicBool::new(false)),
                form_req_rx,
                closed_tx,
                form_req_tx: _form_req_tx.clone(),
                form_source: None,
                child_theme: None,
                child_interpreter_setup: None,
                indexed_engine: Default::default(),
                shared_rust_bridge: None,
                fx_entrance: FxSpec::default(),
                fx_exit: FxSpec::default(),
                fx_restore: false,
                theme_pack: None,
                surface_theme: cobolt_forms::surface_theme::liquid_glass(),
                icon_path: None,
                title_fallback: String::new(),
                hooks: Box::new(NoHooks),
                surface,
            });
            host
        }

        let ctrl_of = |h: &FormHost, id: &str| -> cobolt_forms::Control {
            h.root
                .controls
                .iter()
                .find(|c| c.id == id)
                .expect("control")
                .clone()
        };

        let pane = host_for(Surface::Pane);
        // Designed rects, untouched by the pane slide.
        assert_eq!(
            (ctrl_of(&pane, "SIDE-1-Footer").rect.x, ctrl_of(&pane, "LBL-CLOCK").rect.x),
            (0, 20),
            "the footer subtree is rail, not content — it must not slide"
        );
        assert_eq!(
            ctrl_of(&pane, "BTN-1").rect.x,
            10,
            "…while ordinary content still slides over the rail's column"
        );

        // And the content pass does not draw them.
        let st = crate::state::LiveState {
            state: &pane.root.state,
            anim: &pane.root.anim,
            hidden: Some(&pane.root.footer_ids),
            viewer_docs: None,
                special_names: &pane.root.special_names,
        };
        assert!(
            !st.visible(&ctrl_of(&pane, "SIDE-1-Footer"))
                && !st.visible(&ctrl_of(&pane, "LBL-CLOCK")),
            "the ContentPane must withhold the footer subtree — the rail draws it"
        );
        assert!(
            st.visible(&ctrl_of(&pane, "BTN-1")),
            "…and withhold nothing else"
        );

        // A WINDOW host is untouched: there is no rail chrome there, the
        // SideMenu is an ordinary control and its footer already sits on it.
        let window = host_for(Surface::Window);
        assert!(
            window.root.footer_ids.is_empty(),
            "a window host has no footer band to hand anything to"
        );
        assert_eq!(ctrl_of(&window, "LBL-CLOCK").rect.x, 20);

        println!(
            "049 footer — pane: SIDE-1-Footer/LBL-CLOCK keep x=0/20 and are \
             withheld from the content pass; BTN-1 still slides 210→10; a \
             window host withholds nothing"
        );
    }

    /// R41 — the ContentPane wears the background set in the RAD.
    ///
    /// The pane paints the form's backdrop itself and then hands the ENGINE an
    /// inert one so nothing is painted twice. That inert backdrop was
    /// `#00000000`, and `backdrop_color` maps pure black to the default navy on
    /// purpose — so a form with no background set is still a visible window —
    /// so the engine painted opaque navy straight over the correct fill. The
    /// rail and the breadcrumb resolve their colour by another route, which is
    /// why they honoured the design and the pane alone did not.
    #[test]
    fn an_inert_backdrop_paints_nothing_over_the_panes_own_fill() {
        use cobolt_forms::render::backdrop_color;

        // The trap: pure black is deliberately the navy default, at any length.
        let as_colour = backdrop_color("#00000000", 0);
        assert_eq!(
            (as_colour.r(), as_colour.g(), as_colour.b(), as_colour.a()),
            (20, 22, 45, 255),
            "pure black IS the opaque navy default — a colour alone cannot be inert"
        );

        // What the pane actually hands the engine now: nothing to paint.
        let inert = backdrop_color("#00000000", 100);
        assert_eq!(inert.a(), 0, "transparency 100 is what makes it inert");

        // And the design's own colour is untouched by all of this.
        let designed = backdrop_color("D8D8D8FF", 0);
        assert_eq!(
            (designed.r(), designed.g(), designed.b(), designed.a()),
            (0xD8, 0xD8, 0xD8, 255),
            "a form designed light grey resolves light grey"
        );

        println!(
            "049 R41 — inert backdrop alpha {} (was {}, opaque navy painted \
             over the pane); a D8D8D8FF form still resolves {:?}",
            inert.a(),
            as_colour.a(),
            (designed.r(), designed.g(), designed.b())
        );
    }

    /// 050 AC9/R16 — the surfaces agree on the theme.
    ///
    /// The canvas, the preview, this host and the compiled binary all resolve
    /// through the SAME registry and publish through the same channel, so a
    /// themed form cannot look different depending on where you view it. The
    /// evidence here is the host end: after a real host frame, the form's own
    /// Context carries the theme, and a shadowed control drawn in it casts its
    /// shadow — which under the old code it did not, once the glass style
    /// happened to be Neumorphic.
    #[test]
    fn themed_surfaces_agree() {
        use cobolt_forms::surface_theme;

        // The registry is the single resolution point every surface calls.
        for (form_theme, project_default, want) in [
            (None, None, cobolt_forms::theme::LIQUID_GLASS),
            (None, Some(cobolt_forms::theme::ELEGANCE), cobolt_forms::theme::ELEGANCE),
            (Some(cobolt_forms::theme::ELEGANCE), None, cobolt_forms::theme::ELEGANCE),
            (Some(""), Some(cobolt_forms::theme::ELEGANCE), cobolt_forms::theme::ELEGANCE),
        ] {
            let id = cobolt_forms::theme::resolve_theme_id(form_theme, project_default);
            assert_eq!(
                surface_theme::for_theme_id(&id).id(),
                want,
                "form={form_theme:?} project={project_default:?}"
            );
        }

        // …and the host actually publishes it. Drive one real frame with the
        // Elegance theme, then draw a shadowed Panel in the SAME Context.
        fn shadow_shapes_after_a_host_frame(
            theme: std::sync::Arc<dyn surface_theme::SurfaceTheme>,
            gs: cobolt_forms::model::GlassStyle,
        ) -> usize {
            let (mut app, _pipes) = host_with_surface("none:0:linear", "none:0:linear", false, Surface::Window);
            app.root.surface_theme = theme;
            let ctx = egui::Context::default();
            cobolt_forms::paint::set_glass_style(&ctx, gs);
            let mut input = egui::RawInput::default();
            input.screen_rect =
                Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::new(420.0, 320.0)));
            // The host frame is what publishes the theme onto this Context.
            let _ = frame(&mut app, &ctx, input.clone());

            let mut c = cobolt_forms::Control::new("P", cobolt_forms::ControlType::Panel, 0, 0);
            c.rect = cobolt_forms::model::Rect::new(60, 60, 160, 100);
            c.set_prop("ShadowEnabled", true);
            c.set_prop("ShadowDirection", "South");
            c.set_prop("ShadowDistance", 12i64);
            let face_bottom = 160.0_f32;
            let mut full = ctx.run_ui(input, |root_ui| {
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show_inside(root_ui, |ui| {
                        cobolt_forms::paint::draw_control(
                            ui.painter(),
                            egui::Pos2::ZERO,
                            &c,
                            false,
                            true,
                            1.0,
                            1.0,
                            None,
                        );
                    });
            });
            full.textures_delta.clear();
            fn walk(s: &egui::Shape, bottom: f32, n: &mut usize) {
                match s {
                    egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, bottom, n)),
                    other => {
                        let b = other.visual_bounding_rect();
                        if b.is_positive() && b.max.y > bottom + 1.0 {
                            *n += 1;
                        }
                    }
                }
            }
            let mut n = 0;
            for cs in &full.shapes {
                walk(&cs.shape, face_bottom, &mut n);
            }
            n
        }

        use cobolt_forms::model::GlassStyle as GS;
        let mut rows = Vec::new();
        for gs in [GS::Classic, GS::Enhanced, GS::Neumorphic, GS::NeumorphicDark] {
            let n = shadow_shapes_after_a_host_frame(surface_theme::elegance(), gs);
            assert!(
                n > 0,
                "{gs:?}: the host published a self-contained theme, so the \
                 developer's drop shadow must still be drawn"
            );
            rows.push((gs, n));
        }

        println!("\n  050 AC9 — resolution agrees across surfaces (one registry).");
        println!("  host frame + shadowed Panel, shapes below the face:");
        for (gs, n) in &rows {
            println!("    {:<16} {n}", format!("{gs:?}"));
        }
        println!();
    }

    /// 051 R19/R28 — `ModalOverlayStyle` actually changes what gets PAINTED
    /// over a blocked form, not just whether `disable()` runs. Registers a
    /// modal child of the ROOT directly with the supervisor (no real child
    /// needs to spawn — `root_modal_blocked` only cares that one is
    /// registered), then checks a filled rect in that exact fill color
    /// covers the frame.
    #[test]
    fn modal_overlay_style_changes_the_painted_fill() {
        fn painted_with(style: cobolt_forms::model::ModalOverlayStyle) -> Vec<egui::Color32> {
            let (mut app, _pipes) =
                host_with_surface("none:0:linear", "none:0:linear", false, Surface::Window);
            app.root.modal_overlay_style = style;
            let (reply_tx, _reply_rx) = mpsc::channel();
            let _ = app.supervisor.handle_request(
                cobolt_runtime::form_host::FormRequest::OpenForm {
                    caller: cobolt_runtime::form_host::ROOT_HANDLE.into(),
                    form_id: "CHILD".into(),
                    sync: true,
                    window_state: None,
                    x: None,
                    y: None,
                    width: None,
                    height: None,
                    modal: true,
                    reply: reply_tx,
                },
            );
            assert!(app.root_modal_blocked(), "the registered modal must block the root");

            let ctx = egui::Context::default();
            let mut input = egui::RawInput::default();
            input.screen_rect = Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::Vec2::new(420.0, 320.0),
            ));
            let mut full = ctx.run_ui(input, |root_ui| app.ui_impl(root_ui));
            full.textures_delta.clear();

            fn walk(s: &egui::Shape, out: &mut Vec<egui::Color32>) {
                match s {
                    egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
                    egui::Shape::Rect(r) => out.push(r.fill),
                    _ => {}
                }
            }
            let mut fills = Vec::new();
            for cs in &full.shapes {
                walk(&cs.shape, &mut fills);
            }
            fills
        }

        let semi = painted_with(cobolt_forms::model::ModalOverlayStyle::SemiTransparent);
        assert!(
            semi.contains(&egui::Color32::from_rgba_unmultiplied(60, 60, 64, 64)),
            "SemiTransparent must paint its own overlay fill: {semi:?}"
        );
        let grey = painted_with(cobolt_forms::model::ModalOverlayStyle::Greyed);
        let grey_fill = egui::Color32::from_rgba_unmultiplied(60, 60, 64, 150);
        assert!(
            grey.contains(&grey_fill),
            "Greyed must paint its own, DIFFERENT overlay fill: {grey:?}"
        );
        assert!(
            !semi.contains(&grey_fill),
            "SemiTransparent must not ALSO paint Greyed's fill: {semi:?}"
        );
        // `None` — the default: still blocked, but no layer at all.
        let none = painted_with(cobolt_forms::model::ModalOverlayStyle::None);
        let semi_fill = egui::Color32::from_rgba_unmultiplied(60, 60, 64, 64);
        assert!(
            !none.contains(&grey_fill) && !none.contains(&semi_fill),
            "None must paint no overlay layer at all: {none:?}"
        );
        println!("051 — None paints nothing; SemiTransparent and Greyed each paint their own, distinct overlay fill");
    }

    /// 051 R19/R28 — the SAME check as `modal_overlay_style_changes_the_painted_fill`,
    /// but for a form loaded as a ContentPane OCCUPANT (PowerDemo3's Call Form
    /// demo: `CALL-FORM-DEMO` is opened via the sidebar, not run as its own
    /// window) — the previous test only ever drove `Surface::Window`, so it
    /// never actually exercised `child_frame`'s occupant branch at all
    /// (operator report, 2026-09-19: "the caller is set to Greyed... it
    /// becomes semi-transparent instead").
    #[test]
    fn modal_overlay_style_changes_the_painted_fill_for_an_occupant() {
        fn program_from(src: &str) -> cobolt_ast::program::Program {
            cobolt_parser::parse(cobolt_lexer::tokenize(src, cobolt_lexer::SourceFormat::Free))
                .program
                .expect("parses")
        }
        fn painted_with(style: cobolt_forms::model::ModalOverlayStyle) -> Vec<egui::Color32> {
            let form = cobolt_forms::Form::new("SHELL", "Shell", 640, 480);
            let (ev_tx, _ev_rx) = mpsc::channel();
            let (input_tx, _input_rx) = mpsc::channel();
            let (_state_tx, state_rx) = mpsc::channel();
            let (_display_tx, display_rx) = mpsc::channel();
            let (form_req_tx, form_req_rx) = mpsc::channel();
            let (closed_tx, _closed_rx) = mpsc::channel();
            let source: Option<FormSource> = Some(Box::new(move |id: &str| {
                if id.eq_ignore_ascii_case("CALLER") {
                    let mut form = cobolt_forms::Form::new("CALLER", "Caller", 320, 200);
                    form.modal_overlay_style = style;
                    Ok((form, program_from(
                        "IDENTIFICATION DIVISION.\nPROGRAM-ID. CALLER.\n\
                         DATA DIVISION.\nWORKING-STORAGE SECTION.\n\
                         01 EVT PIC X(30).\n01 CTL PIC X(30).\n\
                         PROCEDURE DIVISION.\n    \
                         CALL \"COBOL-WAIT-EVENT\" USING EVT CTL.\n",
                    )))
                } else {
                    Err(format!("no form named '{id}'"))
                }
            }));
            let (mut host, _form) = FormHost::new(FormHostConfig {
                form,
                flat: Vec::new(),
                state: HashMap::new(),
                ev_tx,
                input_tx,
                state_rx,
                display_rx,
                pending: Arc::new(AtomicUsize::new(0)),
                finished: Arc::new(AtomicBool::new(false)),
                form_req_rx,
                closed_tx,
                form_req_tx: form_req_tx.clone(),
                form_source: source,
                child_theme: None,
                child_interpreter_setup: None,
                indexed_engine: Default::default(),
                shared_rust_bridge: None,
                fx_entrance: cobolt_forms::window_fx::FxSpec::default(),
                fx_exit: cobolt_forms::window_fx::FxSpec::default(),
                fx_restore: false,
                theme_pack: None,
                surface_theme: cobolt_forms::surface_theme::liquid_glass(),
                icon_path: None,
                title_fallback: String::new(),
                hooks: Box::new(NoHooks),
                surface: Surface::Window,
            });

            host.ensure_occupant("CALLER").expect("occupant builds");
            host.show_occupant(Some("CALLER"));
            let (reply_tx, _reply_rx) = mpsc::channel();
            let _ = host.supervisor.handle_request(
                cobolt_runtime::form_host::FormRequest::OpenForm {
                    caller: host.pane.occupants.get("CALLER").unwrap().handle.clone(),
                    form_id: "CHILD".into(),
                    sync: true,
                    window_state: None,
                    x: None,
                    y: None,
                    width: None,
                    height: None,
                    modal: true,
                    reply: reply_tx,
                },
            );
            assert!(host.root_modal_blocked(), "the registered modal must block the occupant");

            let ctx = egui::Context::default();
            let mut input = egui::RawInput::default();
            input.screen_rect = Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::Vec2::new(640.0, 480.0),
            ));
            // Exactly what `ShellApp::ui` does while blocked: the WHOLE root
            // `Ui` is disabled first. egui's `Ui::disable` multiplies the
            // painter's opacity by `disabled_alpha` (0.5), and every child
            // `Ui` inherits that painter — so an overlay painted through
            // `panel_ui.painter()` arrived at HALF its alpha, and Greyed and
            // SemiTransparent both read as the same faint wash under the
            // sidebar while a standalone run (root never disabled) showed
            // the real fill. This test only reproduced the symptom once it
            // disabled the root like the shell does.
            let mut full = ctx.run_ui(input, |root_ui| {
                root_ui.disable();
                host.ui_impl(root_ui);
            });
            full.textures_delta.clear();

            fn walk(s: &egui::Shape, out: &mut Vec<egui::Color32>) {
                match s {
                    egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
                    egui::Shape::Rect(r) => out.push(r.fill),
                    _ => {}
                }
            }
            let mut fills = Vec::new();
            for cs in &full.shapes {
                walk(&cs.shape, &mut fills);
            }
            fills
        }

        let semi = painted_with(cobolt_forms::model::ModalOverlayStyle::SemiTransparent);
        assert!(
            semi.contains(&egui::Color32::from_rgba_unmultiplied(60, 60, 64, 64)),
            "an occupant's SemiTransparent must paint its own overlay fill at FULL \
             strength even under the shell's disabled root Ui: {semi:?}"
        );
        let grey = painted_with(cobolt_forms::model::ModalOverlayStyle::Greyed);
        let grey_fill = egui::Color32::from_rgba_unmultiplied(60, 60, 64, 150);
        assert!(
            grey.contains(&grey_fill),
            "an occupant's Greyed must paint its own, DIFFERENT overlay fill: {grey:?}"
        );
        assert!(
            !semi.contains(&grey_fill),
            "an occupant's SemiTransparent must not ALSO paint Greyed's fill: {semi:?}"
        );
        let none = painted_with(cobolt_forms::model::ModalOverlayStyle::None);
        let semi_fill = egui::Color32::from_rgba_unmultiplied(60, 60, 64, 64);
        assert!(
            !none.contains(&grey_fill) && !none.contains(&semi_fill),
            "an occupant's None must paint no overlay layer at all: {none:?}"
        );
        println!("051 — an occupant's None paints nothing; SemiTransparent and Greyed each paint their own, distinct overlay fill");
    }

    /// One headless frame; returns the ROOT viewport's commands.
    fn frame(app: &mut FormHost, ctx: &egui::Context, input: egui::RawInput) -> Vec<egui::ViewportCommand> {
        let mut full = ctx.run_ui(input, |root_ui| app.ui_impl(root_ui));
        full.textures_delta.clear();
        full.viewport_output
            .get(&egui::ViewportId::ROOT)
            .map(|o| o.commands.clone())
            .unwrap_or_default()
    }

    fn drain_events(pipes: &Pipes) -> Vec<(String, String)> {
        pipes
            .ev_rx
            .try_iter()
            .map(|e| (e.ctrl_id, e.event_id))
            .collect()
    }

    /// One click on a Switch must send exactly ONE `onClick`.
    ///
    /// The operator's handler DISPLAYed on entry and printed two lines per
    /// click (2026-08-21). This drives a real press+release through the same
    /// path a running form takes and counts what reaches the interpreter's
    /// event channel.
    #[test]
    fn one_click_sends_one_onclick() {
        let form = cobolt_forms::Form::new("MAIN", "Main", 320, 200);
        // INSIDE a container, like the operator's Switch-1 (parent="Panel-8").
        // A parentless control never showed the fault: it is drawn once, so it
        // reports one click however many passes there are.
        let mut panel =
            cobolt_forms::Control::new("Panel-8", cobolt_forms::ControlType::Panel, 0, 0);
        panel.rect = cobolt_forms::model::Rect::new(0, 0, 200, 100);
        let mut sw =
            cobolt_forms::Control::new("Switch-1", cobolt_forms::ControlType::Switch, 10, 10);
        sw.rect = cobolt_forms::model::Rect::new(10, 10, 60, 24);
        sw.parent = Some("Panel-8".into());
        // BOUND, like the operator's Switch-1. `onClick` is emitted only for a
        // control that binds a handler, so an unbound probe is blind to the
        // double-fire this test exists for — which is precisely why the first
        // version of it passed while the real form was broken.
        sw.ensure_event("onClick");
        let flat = vec![panel, sw];
        let (ev_tx, ev_rx) = mpsc::channel();
        let (input_tx, _input_rx) = mpsc::channel();
        let (_state_tx, state_rx) = mpsc::channel();
        let (_display_tx, display_rx) = mpsc::channel();
        let (_form_req_tx, form_req_rx) = mpsc::channel();
        let (closed_tx, _closed_rx) = mpsc::channel();
        let (mut host, _form) = FormHost::new(FormHostConfig {
            form,
            flat,
            state: HashMap::new(),
            ev_tx,
            input_tx,
            state_rx,
            display_rx,
            pending: Arc::new(AtomicUsize::new(0)),
            finished: Arc::new(AtomicBool::new(false)),
            form_req_rx,
            closed_tx,
            form_req_tx: _form_req_tx.clone(),
            form_source: None,
            child_theme: None,
            child_interpreter_setup: None,
            indexed_engine: Default::default(),
            shared_rust_bridge: None,
            fx_entrance: FxSpec::default(),
            fx_exit: FxSpec::default(),
            fx_restore: false,
            theme_pack: None,
            surface_theme: cobolt_forms::surface_theme::liquid_glass(),
            icon_path: None,
            title_fallback: String::new(),
            hooks: Box::new(NoHooks),
            surface: Surface::Window,
        });
        let pipes = Pipes {
            ev_rx,
            _input_rx,
            _state_tx,
            _display_tx,
            finished: Arc::new(AtomicBool::new(false)),
            _form_req_tx,
            _closed_rx,
        };
        let ctx = egui::Context::default();

        // The host ignores interaction for its first 450 ms (the entrance
        // window), so a click before that forwards nothing at all.
        let _ = frame(&mut host, &ctx, raw());
        std::thread::sleep(Duration::from_millis(500));
        for _ in 0..2 {
            let _ = frame(&mut host, &ctx, raw());
        }
        let _ = drain_events(&pipes);

        let at = egui::pos2(30.0, 30.0);
        let mut down = raw();
        down.events = vec![
            egui::Event::PointerMoved(at),
            egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            },
        ];
        let _ = frame(&mut host, &ctx, down);
        let mut up = raw();
        up.events = vec![egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        }];
        let _ = frame(&mut host, &ctx, up);
        // Quiet frames, in case a duplicate arrives one frame late.
        for _ in 0..3 {
            let _ = frame(&mut host, &ctx, raw());
        }

        let evs = drain_events(&pipes);
        let clicks: Vec<_> = evs
            .iter()
            .filter(|(_, e)| e.eq_ignore_ascii_case("onClick"))
            .collect();
        assert_eq!(
            clicks.len(),
            1,
            "one click must send exactly one onClick; got {evs:?}"
        );
    }

    /// 051 R19/R28 — a click on a form BLOCKED by its own modal child must
    /// reach no handler. `ui.disable()` refuses input to egui widgets, but the
    /// engine detects a control's click from RAW pointer state (`render.rs`,
    /// "One press, one click"), which `disable()` never touched: the `onClick`
    /// was still emitted, queued on the interpreter's event channel while it
    /// sat inside `OpenFormSync`, and replayed the moment the modal closed —
    /// each click on the caller's "open" button while the modal was up opened
    /// it once more (operator, 2026-09-19). The same press is driven twice:
    /// once blocked (nothing may arrive), once released (exactly one arrives —
    /// so the silence was the block, not a dead pipe).
    #[test]
    fn a_click_on_a_blocked_form_reaches_no_handler() {
        let form = cobolt_forms::Form::new("MAIN", "Main", 320, 200);
        let mut btn =
            cobolt_forms::Control::new("Btn-Open", cobolt_forms::ControlType::Button, 10, 10);
        btn.rect = cobolt_forms::model::Rect::new(10, 10, 80, 30);
        btn.ensure_event("onClick");
        let (ev_tx, ev_rx) = mpsc::channel();
        let (input_tx, _input_rx) = mpsc::channel();
        let (_state_tx, state_rx) = mpsc::channel();
        let (_display_tx, display_rx) = mpsc::channel();
        let (_form_req_tx, form_req_rx) = mpsc::channel();
        let (closed_tx, _closed_rx) = mpsc::channel();
        let (mut host, _form) = FormHost::new(FormHostConfig {
            form,
            flat: vec![btn],
            state: HashMap::new(),
            ev_tx,
            input_tx,
            state_rx,
            display_rx,
            pending: Arc::new(AtomicUsize::new(0)),
            finished: Arc::new(AtomicBool::new(false)),
            form_req_rx,
            closed_tx,
            form_req_tx: _form_req_tx.clone(),
            form_source: None,
            child_theme: None,
            child_interpreter_setup: None,
            indexed_engine: Default::default(),
            shared_rust_bridge: None,
            fx_entrance: FxSpec::default(),
            fx_exit: FxSpec::default(),
            fx_restore: false,
            theme_pack: None,
            surface_theme: cobolt_forms::surface_theme::liquid_glass(),
            icon_path: None,
            title_fallback: String::new(),
            hooks: Box::new(NoHooks),
            surface: Surface::Window,
        });
        let pipes = Pipes {
            ev_rx,
            _input_rx,
            _state_tx,
            _display_tx,
            finished: Arc::new(AtomicBool::new(false)),
            _form_req_tx,
            _closed_rx,
        };
        let ctx = egui::Context::default();

        // Past the entrance window, like `one_click_sends_one_onclick`.
        let _ = frame(&mut host, &ctx, raw());
        std::thread::sleep(Duration::from_millis(500));
        for _ in 0..2 {
            let _ = frame(&mut host, &ctx, raw());
        }
        let _ = drain_events(&pipes);

        // Register a modal child of the ROOT directly with the supervisor —
        // `root_modal_blocked` only cares that one is registered.
        let (reply_tx, _reply_rx) = mpsc::channel();
        let _ = host.supervisor.handle_request(
            cobolt_runtime::form_host::FormRequest::OpenForm {
                caller: cobolt_runtime::form_host::ROOT_HANDLE.into(),
                form_id: "CHILD".into(),
                sync: true,
                window_state: None,
                x: None,
                y: None,
                width: None,
                height: None,
                modal: true,
                reply: reply_tx,
            },
        );
        assert!(host.root_modal_blocked(), "the registered modal must block the root");

        let press = |host: &mut FormHost| {
            let at = egui::pos2(30.0, 20.0);
            let mut down = raw();
            down.events = vec![
                egui::Event::PointerMoved(at),
                egui::Event::PointerButton {
                    pos: at,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                },
            ];
            let _ = frame(host, &ctx, down);
            let mut up = raw();
            up.events = vec![egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }];
            let _ = frame(host, &ctx, up);
            for _ in 0..3 {
                let _ = frame(host, &ctx, raw());
            }
        };
        let clicks = |evs: &[(String, String)]| {
            evs.iter()
                .filter(|(_, e)| e.eq_ignore_ascii_case("onClick"))
                .count()
        };

        press(&mut host);
        let blocked_evs = drain_events(&pipes);
        assert_eq!(
            clicks(&blocked_evs),
            0,
            "a click on a modal-blocked form must reach no handler; got {blocked_evs:?}"
        );

        // Release the block through the ordinary close path and repeat the
        // identical press: now exactly one onClick must arrive.
        let modal = host
            .supervisor
            .modal_children_of(cobolt_runtime::form_host::ROOT_HANDLE)
            .into_iter()
            .next()
            .expect("the modal child is registered");
        host.supervisor.try_close(&modal);
        assert!(!host.root_modal_blocked(), "closing the modal releases the root");
        let _ = drain_events(&pipes);
        press(&mut host);
        let free_evs = drain_events(&pipes);
        assert_eq!(
            clicks(&free_evs),
            1,
            "the same press on the released form must send exactly one onClick; got {free_evs:?}"
        );
        println!(
            "blocked form: {} onClick forwarded; released form: {} onClick forwarded",
            clicks(&blocked_evs),
            clicks(&free_evs)
        );
    }

    /// An OBSERVER event fires when the INTERPRETER changes a value, on the ROOT
    /// form — the one `rcrun run-form` shows.
    ///
    /// This is the reported bug (operator, 2026-08-17). 1.61.71 added observer
    /// events to `FormBody::child_frame`, the path a CHILD window and a
    /// ContentPane occupant take, and the root form takes neither: a Timer doing
    /// `MOVE 5 TO KNOB-1::Value` in the main form still fired nothing at all —
    /// exactly the symptom that change set out to cure. Both paths now go through
    /// `FormBody::apply_interpreter_update`, so neither can be fixed alone again.
    #[test]
    fn an_interpreter_write_fires_the_observer_event_on_the_root_form() {
        fn knob_host() -> (FormHost, Pipes) {
            let form = cobolt_forms::Form::new("MAIN", "Main", 320, 200);
            let mut knob = cobolt_forms::Control::new("KNOB-1", cobolt_forms::ControlType::Knob, 10, 10);
            knob.set_prop("Value", cobolt_forms::PropValue::String("0".into()));
            let (ev_tx, ev_rx) = mpsc::channel();
            let (input_tx, _input_rx) = mpsc::channel();
            let (_state_tx, state_rx) = mpsc::channel();
            let (_display_tx, display_rx) = mpsc::channel();
            let (_form_req_tx, form_req_rx) = mpsc::channel();
            let (closed_tx, _closed_rx) = mpsc::channel();
            let finished = Arc::new(AtomicBool::new(false));
            let (host, _form) = FormHost::new(FormHostConfig {
                form,
                flat: vec![knob],
                state: HashMap::new(),
                ev_tx,
                input_tx,
                state_rx,
                display_rx,
                pending: Arc::new(AtomicUsize::new(0)),
                finished: Arc::clone(&finished),
                form_req_rx,
                closed_tx,
                form_req_tx: _form_req_tx.clone(),
                form_source: None,
                child_theme: None,
                child_interpreter_setup: None,
                indexed_engine: Default::default(),
                shared_rust_bridge: None,
                fx_entrance: FxSpec::parse(""),
                fx_exit: FxSpec::parse(""),
                fx_restore: false,
                theme_pack: None,
                surface_theme: cobolt_forms::surface_theme::liquid_glass(),
                icon_path: None,
                title_fallback: String::new(),
                hooks: Box::new(NoHooks),
                surface: Surface::Window,
            });
            (
                host,
                Pipes {
                    ev_rx,
                    _input_rx,
                    _state_tx,
                    _display_tx,
                    finished,
                    _form_req_tx,
                    _closed_rx,
                },
            )
        }

        let ctx = egui::Context::default();
        ctx.set_fonts(egui::FontDefinitions::default());
        let (mut app, pipes) = knob_host();

        // Warm-up: input is ignored for a moment after a form appears, and the
        // lifecycle events go out on the first frames. Clear them.
        frame(&mut app, &ctx, raw());
        std::thread::sleep(Duration::from_millis(220));
        frame(&mut app, &ctx, raw());
        let _ = drain_events(&pipes);

        // A Timer handler raising the knob: exactly `MOVE 5 TO KNOB-1::Value`.
        pipes
            ._state_tx
            .send(StateUpdate::new("KNOB-1".to_owned(), "Value".to_owned(), "5".to_owned()))
            .expect("the host is listening");
        frame(&mut app, &ctx, raw());

        let events = drain_events(&pipes);
        for want in ["onValueChanged", "onChange"] {
            assert!(
                events
                    .iter()
                    .any(|(id, ev)| id == "KNOB-1" && ev == want),
                "a write to KNOB-1::Value must fire {want}, got {events:?}"
            );
        }
        // …and the value itself landed, so the knob is drawn where it was put.
        assert_eq!(
            app.root
                .state
                .get("KNOB-1")
                .and_then(|s| s.props.iter().find(|(k, _)| *k == "Value").map(|(_, v)| v.as_str())),
            Some("5")
        );

        // Writing the SAME value again is not a change, so it fires nothing — an
        // observer that re-fires is a spurious event, and a handler that writes
        // the property it was woken for would otherwise loop.
        pipes
            ._state_tx
            .send(StateUpdate::new("KNOB-1".to_owned(), "Value".to_owned(), "5".to_owned()))
            .expect("still listening");
        frame(&mut app, &ctx, raw());
        assert!(
            drain_events(&pipes).is_empty(),
            "a write of the same value must fire nothing"
        );

        // A PASSIVE event is never raised by a write: there is no user act.
        pipes
            ._state_tx
            .send(StateUpdate::new("KNOB-1".to_owned(), "Value".to_owned(), "7".to_owned()))
            .expect("still listening");
        frame(&mut app, &ctx, raw());
        let events = drain_events(&pipes);
        assert!(
            !events.iter().any(|(_, ev)| ev == "onClick"),
            "a write is not a click: {events:?}"
        );

        println!(
            "observer events on the ROOT form — MOVE 5 TO KNOB-1::Value fires \
             onValueChanged AND onChange on the main form's path (it fired NOTHING \
             before: 1.61.71 reached only the child-window path); re-writing 5 fires \
             nothing; writing 7 fires the observers and never onClick"
        );
    }

    /// A Timer whose handler writes a property must keep ticking.
    ///
    /// Reported 2026-08-17: "timer events are dying after some dozens of times. It
    /// stops silently, no warnings, just stops."
    ///
    /// Tick coalescing dropped a due tick whenever ANY event was outstanding. That
    /// was harmless until observer events arrived (1.61.75): a Timer handler that
    /// raises a Gauge or sets a Label queues one or two `onChange`/`onValueChanged`
    /// events per tick, so there was nearly always something outstanding, and the
    /// next tick went in the bin. Silently — a dropped tick has nothing to report.
    ///
    /// The guard now waits until the interpreter is genuinely behind.
    /// `CloneEvents` off: only the designed card (instance 1) of a repeating
    /// group runs the handlers; a click on a clone is not dispatched. On (the
    /// default), every card fires with its CONTROL-ARRAY-INDEX (property
    /// audit, 2026-09-25).
    #[test]
    fn clone_events_off_keeps_the_clones_from_firing() {
        let pending = Arc::new(AtomicUsize::new(0));
        let (ev_tx, ev_rx) = mpsc::channel();
        let (input_tx, _input_rx) = mpsc::channel();
        let mut body = timer_body(ev_tx, input_tx, pending);
        let mut group = cobolt_forms::Control::new("CARD", cobolt_forms::ControlType::GroupBox, 0, 0);
        group.set_prop("IsRepeatingGroup", cobolt_forms::model::PropValue::Bool(true));
        group.set_prop("CloneEvents", cobolt_forms::model::PropValue::Bool(false));
        body.controls.push(group);
        let click = |inst: usize| cobolt_forms::render::UiEvent {
            ctrl_id: format!("CARD.CARD-{inst}.BTN"),
            event: "onClick".to_owned(),
            value: None,
        };
        body.forward_interaction(&[], vec![click(1), click(2)], false);
        let sent: Vec<(String, usize)> = ev_rx.try_iter().map(|e| (e.ctrl_id, e.instance_index)).collect();
        assert_eq!(sent, vec![("BTN".to_owned(), 1)], "only the designed card fires");

        body.controls.last_mut().unwrap().set_prop("CloneEvents", cobolt_forms::model::PropValue::Bool(true));
        body.forward_interaction(&[], vec![click(1), click(2)], false);
        let sent: Vec<(String, usize)> = ev_rx.try_iter().map(|e| (e.ctrl_id, e.instance_index)).collect();
        assert_eq!(sent, vec![("BTN".to_owned(), 1), ("BTN".to_owned(), 2)], "every card fires");
    }

    #[test]
    fn a_timer_is_not_starved_by_the_events_its_own_handler_causes() {
        let body_pending = Arc::new(AtomicUsize::new(0));
        let (ev_tx, ev_rx) = mpsc::channel();
        let (input_tx, _input_rx) = mpsc::channel();

        // A tick, and the two observer events a handler writing one value causes.
        let tick = || cobolt_forms::render::UiEvent {
            ctrl_id: "TMR-1".to_owned(),
            event: "onTick".to_owned(),
            value: None,
        };
        let drain = |rx: &mpsc::Receiver<FormEvent>| -> Vec<String> {
            rx.try_iter().map(|e| e.event_id).collect()
        };

        let mut body = timer_body(ev_tx, input_tx, Arc::clone(&body_pending));

        // One or two events outstanding is a handler keeping up, not a backlog:
        // the tick must go through.
        for outstanding in [0usize, 1, 2, FormBody::TICK_COALESCE_BACKLOG - 1] {
            body_pending.store(outstanding, Ordering::Relaxed);
            let _ = drain(&ev_rx);
            body.forward_interaction(&[], vec![tick()], false);
            assert_eq!(
                drain(&ev_rx),
                vec!["onTick".to_owned()],
                "with {outstanding} event(s) outstanding the tick must still be sent"
            );
        }

        // Genuinely behind: the tick is coalesced away, which is the rule's point.
        body_pending.store(FormBody::TICK_COALESCE_BACKLOG, Ordering::Relaxed);
        let _ = drain(&ev_rx);
        body.forward_interaction(&[], vec![tick()], false);
        assert!(
            drain(&ev_rx).is_empty(),
            "a handler {} events behind must not be given more ticks",
            FormBody::TICK_COALESCE_BACKLOG
        );

        // …and a USER event is never dropped, however far behind the handler is.
        body_pending.store(FormBody::TICK_COALESCE_BACKLOG * 10, Ordering::Relaxed);
        let _ = drain(&ev_rx);
        body.forward_interaction(
            &[],
            vec![cobolt_forms::render::UiEvent {
                ctrl_id: "BTN-1".to_owned(),
                event: "onClick".to_owned(),
                value: None,
            }],
            false,
        );
        assert_eq!(
            drain(&ev_rx),
            vec!["onClick".to_owned()],
            "a click is never coalesced — only ticks are"
        );

        println!(
            "timer starvation — a tick survives 0, 1, 2 and {} outstanding events (a \
             handler that writes one value queues two of them, which is why ANY \
             outstanding event used to kill the timer); it is coalesced only once the \
             interpreter is {} events behind, and a click is never coalesced at all",
            FormBody::TICK_COALESCE_BACKLOG - 1,
            FormBody::TICK_COALESCE_BACKLOG
        );
    }

    /// **A motion event nobody handles is never queued.**
    ///
    /// `onMouseMove`/`onPointerMove` fire twice a frame for as long as the
    /// pointer moves. The interpreter retires ONE event per
    /// `COBOL-WAIT-EVENT`, so while it is busy — a synchronous
    /// `AgentObject::Ask` is the case that found this — the queue grows by
    /// ~120 a second and never comes back. The form answered the first few
    /// questions and then stopped (operator, 2026-09-07).
    ///
    /// Only motion is filtered, and only when unbound: every other event is a
    /// discrete act and stays exactly as it was.
    #[test]
    fn an_unbound_motion_event_is_not_queued_but_everything_else_still_is() {
        let (ev_tx, ev_rx) = mpsc::channel();
        let (input_tx, _input_rx) = mpsc::channel();
        let pending = Arc::new(AtomicUsize::new(0));
        let mut body = timer_body(ev_tx, input_tx, Arc::clone(&pending));
        body.motion_bound = std::collections::HashSet::new(); // binds nothing

        for ev in ["onMouseMove", "onPointerMove"] {
            body.send_event(FormEvent::new("TIMER-FORM".to_owned(), ev));
        }
        assert!(
            ev_rx.try_iter().next().is_none(),
            "an unbound motion event reached the interpreter"
        );
        assert_eq!(
            pending.load(Ordering::Relaxed),
            0,
            "a dropped event must not be counted against the backlog"
        );

        // The filter is motion-only: a click with no handler still goes, exactly
        // as before. Narrowing THIS would change dispatch for every control.
        for ev in ["onClick", "onKeyDown", "onTick", "onChange"] {
            body.send_event(FormEvent::new("TIMER-FORM".to_owned(), ev));
        }
        let got: Vec<String> = ev_rx.try_iter().map(|e| e.event_id).collect();
        assert_eq!(got, vec!["onClick", "onKeyDown", "onTick", "onChange"]);
        assert_eq!(pending.load(Ordering::Relaxed), 4);
    }

    /// A form that DOES bind motion still gets every one of them: the developer
    /// asked for the firehose.
    #[test]
    fn a_bound_motion_event_is_delivered_whatever_the_spelling() {
        let (ev_tx, ev_rx) = mpsc::channel();
        let (input_tx, _input_rx) = mpsc::channel();
        let pending = Arc::new(AtomicUsize::new(0));
        let mut body = timer_body(ev_tx, input_tx, Arc::clone(&pending));
        body.motion_bound = motion_bindings(
            "TIMER-FORM",
            &[cobolt_forms::model::EventBinding::new("onMouseMove", "")],
            &[],
        );

        // A COBOL word arrives upper-cased and the designer's spelling is what
        // the .cfrm holds — the key folds both, so either reaches the handler.
        for ctrl in ["TIMER-FORM", "Timer-Form"] {
            body.send_event(FormEvent::new(ctrl.to_owned(), "onMouseMove"));
        }
        // …and the sibling motion event, unbound, is still dropped.
        body.send_event(FormEvent::new("TIMER-FORM".to_owned(), "onPointerMove"));

        let got: Vec<String> = ev_rx.try_iter().map(|e| e.event_id).collect();
        assert_eq!(got, vec!["onMouseMove", "onMouseMove"]);
        assert_eq!(pending.load(Ordering::Relaxed), 2);
    }

    /// The set is built from the form's own bindings AND every control's.
    #[test]
    fn motion_bindings_reads_the_form_and_its_controls() {
        let mut btn = cobolt_forms::Control::new("Btn-Go", cobolt_forms::ControlType::Button, 0, 0);
        btn.events = vec![
            cobolt_forms::model::EventBinding::new("onPointerMove", ""),
            // Not motion — must not enter the set.
            cobolt_forms::model::EventBinding::new("onClick", ""),
        ];
        let set = motion_bindings(
            "MY-FORM",
            &[cobolt_forms::model::EventBinding::new("onMouseMove", "")],
            &[btn],
        );
        assert!(set.contains(&motion_key("my-form", "ONMOUSEMOVE")));
        assert!(set.contains(&motion_key("BTN-GO", "onpointermove")));
        assert!(!set.contains(&motion_key("Btn-Go", "onClick")));
        assert_eq!(set.len(), 2);
    }

    /// **A form's events reach only that form's interpreter** (spec 061 R7).
    ///
    /// Every form body owns its own event channel, which is why a click on
    /// one form reaches that form's program and no other — and why the form
    /// the developer is *not* working in sits in its wait state inside
    /// `COBOL-WAIT-EVENT`, receiving nothing, until they come back to it.
    ///
    /// That behaviour predates spec 061 and nothing in it changes: this guard
    /// exists so the wait-state requirement cannot be broken quietly, by
    /// routing interaction through a shared or root channel on the way to
    /// making the debugger follow the program.
    ///
    /// (`timer_body` is reused for both: what is under test is which channel
    /// receives, not what the form contains.)
    #[test]
    fn a_forms_events_reach_only_that_forms_interpreter() {
        let (a_tx, a_rx) = mpsc::channel::<FormEvent>();
        let (b_tx, b_rx) = mpsc::channel::<FormEvent>();
        let (in_tx, _in_rx) = mpsc::channel::<StateUpdate>();
        let pending = Arc::new(AtomicUsize::new(0));
        let mut clicked = timer_body(a_tx, in_tx.clone(), Arc::clone(&pending));
        let _waiting = timer_body(b_tx, in_tx, Arc::clone(&pending));

        clicked.forward_interaction(
            &[],
            vec![cobolt_forms::render::UiEvent {
                ctrl_id: "BTN".to_owned(),
                event: "onClick".to_owned(),
                value: None,
            }],
            false,
        );

        assert!(
            a_rx.try_recv().is_ok(),
            "the form the click happened on receives it"
        );
        assert!(
            b_rx.try_recv().is_err(),
            "the other form receives NOTHING — it is waiting, and a click on \
             a different window is not its event"
        );
    }

    /// A body with one Timer, wired to test channels.
    fn timer_body(
        ev_tx: mpsc::Sender<FormEvent>,
        input_tx: mpsc::Sender<StateUpdate>,
        pending: Arc<AtomicUsize>,
    ) -> FormBody {
        let (_state_tx, state_rx) = mpsc::channel();
        let (_display_tx, display_rx) = mpsc::channel();
        let timer = cobolt_forms::Control::new("TMR-1", cobolt_forms::ControlType::Timer, 0, 0);
        FormBody {
            drawn_reported: false,
            form_name: "TIMER-FORM".to_owned(),
            title_visible: true,
            corner_radius: 0,
            see_through_window: false,
                pane_window: None,
            owns_window: false,
            last_window_crumb: None,
            footer_ids: std::collections::HashSet::new(),
            theme_pack: None,
            surface_theme: cobolt_forms::surface_theme::liquid_glass(),
            glass_style: cobolt_forms::model::GlassStyle::default(),
            motion_bound: std::collections::HashSet::new(),
            controls: vec![timer],
            special_names: String::new(),
            state: HashMap::new(),
            bg_hex: String::new(),
            bg_gradient_enabled: false,
            bg_gradient_start: String::new(),
            bg_gradient_end: String::new(),
            bg_gradient_direction: String::new(),
            transparency: 0,
            bg_image: String::new(),
            bg_mode: cobolt_forms::model::BgImageMode::default(),
            use_theme_background: false,
            modal_overlay_style: cobolt_forms::model::ModalOverlayStyle::default(),
            form_size: egui::vec2(320.0, 200.0),
            responsive: None,
            responsive_off: None,
            ev_tx,
            input_tx,
            state_rx,
            display_rx,
            pending,
            finished: Arc::new(AtomicBool::new(false)),
            start: Instant::now(),
            lifecycle_sent: true,
            db_dumped: false,
            form_object: "TIMER-FORM".to_owned(),
            anim: cobolt_forms::anim::AnimRuntime::default(),
            anim_started: true,
            last_frame: None,
            hovered: std::collections::HashSet::new(),
            parked_timer_clocks: HashMap::new(),
            toolbar_runner: cobolt_forms::toolbar_actions::Runner::default(),
            pending_save_as: Vec::new(),
            pending_os_handoff: Vec::new(),
            os_handoff: OsHandoffChannel::default(),
            action_notice: None,
            last_control_rects: HashMap::new(),
                last_layout: None,
                mirrored: None,
                breakpoint_reported: None,
                font_scale_reported: None,
                snackbars: Default::default(),
                viewer_sessions: Default::default(),
        }
    }

    /// Operator (2026-09-27): PowerChat's Documents upload — "I can select the
    /// file, but it does not get processed nor even appear in the folder". The
    /// form sets the zone's `DestinationFolder` from COBOL at run time; the
    /// click-to-browse intake read only the DESIGNED value (blank), so the
    /// picked file was accepted with nowhere to go and never copied. The
    /// live value must win, exactly as it does on the drag-drop path.
    #[test]
    fn a_browsed_file_goes_to_the_destination_the_cobol_set() {
        let (ev_tx, ev_rx) = mpsc::channel();
        let (input_tx, _input_rx) = mpsc::channel();
        let mut body = timer_body(ev_tx, input_tx, Arc::new(AtomicUsize::new(0)));
        // Designed with NO destination — PowerChat's documents-form exactly.
        let zone = cobolt_forms::Control::new(
            "DROP-BROWSE-LIVE",
            cobolt_forms::ControlType::FileDropZone,
            0,
            0,
        );
        body.controls.push(zone);

        let root = std::env::temp_dir().join(format!(
            "fdz-browse-live-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let source = root.join("picked.txt");
        std::fs::write(&source, b"hello").unwrap();
        let dest = root.join("documents").join("Policies");
        // What `MOVE ... TO Drop-Docs::DestinationFolder` leaves in the state.
        body.state_entry_mut("DROP-BROWSE-LIVE")
            .set("DestinationFolder", dest.display().to_string());

        crate::file_dialog::answer_for_test(
            "filedropzone:DROP-BROWSE-LIVE",
            Some(source.clone()),
        );
        let ctx = egui::Context::default();
        let mut full = ctx.run_ui(Default::default(), |ui| {
            let ctx2 = ui.ctx().clone();
            body.run_platform_requests(&ctx2, &[], &[], &[], None, None);
        });
        full.textures_delta.clear();

        let copied = dest.join("picked.txt");
        assert!(
            copied.is_file(),
            "the picked file lands in the folder the COBOL set, not nowhere"
        );
        assert_eq!(std::fs::read(&copied).unwrap(), b"hello");
        let fired: Vec<String> = ev_rx.try_iter().map(|e| e.event_id).collect();
        assert!(
            fired.iter().any(|e| e == "onFilesDropped"),
            "onFilesDropped fires so the form can index it — got {fired:?}"
        );
        println!(
            "browse intake — designed DestinationFolder blank, live one set: \
             1 file picked, 1 copied to {}, onFilesDropped fired",
            dest.display()
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Operator (2026-08-23): "copy, paste are doing nothing". Two defects in
    /// one chain: the click that presses the toolbar button SURRENDERS the
    /// text field's focus before the press is executed (egui's
    /// `SurrenderFocusOn::Clicks`), so live focus is always `None`; and an
    /// untouched field had no state entry, so even a resolved Copy copied "".
    /// The pre-press focus is the fallback, and the DESIGNED text is what an
    /// unedited field yields — pinned here end-to-end through
    /// `run_platform_requests`.
    #[test]
    fn copy_uses_the_pre_press_focus_and_the_designed_text() {
        let (ev_tx, _ev_rx) = mpsc::channel();
        let (input_tx, _input_rx) = mpsc::channel();
        let mut body = timer_body(ev_tx, input_tx, Arc::new(AtomicUsize::new(0)));
        let mut txt =
            cobolt_forms::Control::new("TXT-1", cobolt_forms::ControlType::TextBox, 0, 0);
        txt.set_prop(
            "Text",
            cobolt_forms::PropValue::String("HELLO FROM DESIGN".into()),
        );
        body.controls.push(txt);

        let press = [(
            "TB-1".to_owned(),
            "save".to_owned(),
            "copy".to_owned(),
        )];

        // Live focus is None — the post-surrender state — and the pre-press
        // focus names the field the user was in.
        let ctx = egui::Context::default();
        let pre = Some(egui::Id::new(("rt_ctrl", "TXT-1")));
        let mut full = ctx.run_ui(Default::default(), |_root| {
            let ctx2 = _root.ctx().clone();
            body.run_platform_requests(&ctx2, &[], &[], &press, pre, None);
        });
        full.textures_delta.clear();
        let copied = full.platform_output.commands.iter().find_map(|c| match c {
            egui::OutputCommand::CopyText(t) => Some(t.clone()),
            _ => None,
        });
        assert_eq!(
            copied.as_deref(),
            Some("HELLO FROM DESIGN"),
            "copy must reach the pre-press field and its designed text"
        );
        let (msg, is_error, _) = body.action_notice.clone().expect("outcome surfaced");
        assert!(!is_error, "a successful copy is not an error: {msg}");
        assert!(msg.contains("Copied"), "the notice says what happened: {msg}");

        // Without any focus at all, the failure is SURFACED, not silent.
        // A FRESH context: the copy above handed the focus back to the field
        // (which is the point of `restore_caret`), so this one must start
        // from a form where nothing is focused at all.
        body.action_notice = None;
        let ctx = egui::Context::default();
        let mut full = ctx.run_ui(Default::default(), |_root| {
            let ctx2 = _root.ctx().clone();
            body.run_platform_requests(&ctx2, &[], &[], &press, None, None);
        });
        full.textures_delta.clear();
        assert!(
            !full
                .platform_output
                .commands
                .iter()
                .any(|c| matches!(c, egui::OutputCommand::CopyText(_))),
            "no focus, nothing copied"
        );
        let (msg, is_error, _) = body.action_notice.clone().expect("failure surfaced");
        assert!(is_error, "the no-focus failure is visible: {msg}");
        println!("copy: pre-press focus + designed text → copied; no focus → visible failure");
    }

    /// Operator (2026-08-23): a Copy must take only what is SELECTED, and
    /// must hand the field its focus back with the caret right after the last
    /// character copied. This drives the whole chain — a real
    /// `TextEditState` carrying a selection, through `run_platform_requests`,
    /// out to the clipboard command and back to egui's focus and cursor.
    #[test]
    fn copy_takes_only_the_selection_and_hands_the_field_back() {
        let (ev_tx, _ev_rx) = mpsc::channel();
        let (input_tx, _input_rx) = mpsc::channel();
        let mut body = timer_body(ev_tx, input_tx, Arc::new(AtomicUsize::new(0)));
        let mut txt =
            cobolt_forms::Control::new("TXT-1", cobolt_forms::ControlType::TextBox, 0, 0);
        txt.set_prop(
            "Text",
            cobolt_forms::PropValue::String("HELLO WORLD".into()),
        );
        body.controls.push(txt);

        let widget_id = egui::Id::new(("rt_ctrl", "TXT-1"));
        let ctx = egui::Context::default();
        // The developer selected "WORLD" (characters 6..11) before pressing.
        let mut state = egui::text_edit::TextEditState::default();
        state.cursor.set_char_range(Some(egui::text::CCursorRange {
            primary: egui::text::CCursor::new(6usize),
            secondary: egui::text::CCursor::new(11usize),
            h_pos: None,
        }));
        state.store(&ctx, widget_id);

        let press = [("TB-1".to_owned(), "copy".to_owned(), "copy".to_owned())];
        let mut full = ctx.run_ui(Default::default(), |root| {
            let ctx2 = root.ctx().clone();
            // Live focus already surrendered by the press; pre-press focus names the field.
            body.run_platform_requests(&ctx2, &[], &[], &press, Some(widget_id), None);
        });
        full.textures_delta.clear();

        let copied = full.platform_output.commands.iter().find_map(|c| match c {
            egui::OutputCommand::CopyText(t) => Some(t.clone()),
            _ => None,
        });
        assert_eq!(
            copied.as_deref(),
            Some("WORLD"),
            "only the selection reaches the clipboard"
        );
        assert_eq!(
            ctx.memory(|m| m.focused()),
            Some(widget_id),
            "the field gets its focus back"
        );
        let after = egui::text_edit::TextEditState::load(&ctx, widget_id)
            .and_then(|s| s.cursor.char_range())
            .expect("a caret was left behind");
        assert!(after.is_empty(), "the caret selects nothing after a copy");
        assert_eq!(
            after.primary.index.0, 11,
            "caret right after the last character copied"
        );
        println!(
            "copy of \"HELLO WORLD\" with 6..11 selected → clipboard \"WORLD\", focus back on TXT-1, caret at 11"
        );
    }

    // ── Group 3: effects gating (R5–R10) ─────────────────────────────────────

    /// 049 R18/R42/AC8 — the SAME entrance spec that animates a Window host
    /// is inert on a Pane host: live UI from the first frame, and no viewport
    /// command ever leaves the pane.
    #[test]
    fn pane_surface_plays_no_effects_and_issues_no_viewport_commands_049() {
        // A Window host with this spec plays a 3s entrance (the test below).
        let (mut app, _pipes) = host_with_surface(
            "fade:3000:linear",
            "none:0:linear",
            false,
            Surface::Pane,
        );
        let ctx = egui::Context::default();
        assert!(
            app.fx_entrance_done,
            "R18: a pane-hosted form is simply present — no entrance pending"
        );
        // egui emits its own bookkeeping (SetTheme); only WINDOW-affecting
        // commands matter here.
        let window_cmds = |cmds: &[egui::ViewportCommand]| {
            cmds.iter()
                .filter(|c| {
                    matches!(
                        c,
                        egui::ViewportCommand::Close
                            | egui::ViewportCommand::CancelClose
                            | egui::ViewportCommand::Minimized(_)
                            | egui::ViewportCommand::Maximized(_)
                            | egui::ViewportCommand::Fullscreen(_)
                            | egui::ViewportCommand::Decorations(_)
                            | egui::ViewportCommand::Focus
                            | egui::ViewportCommand::OuterPosition(_)
                            | egui::ViewportCommand::InnerSize(_)
                    )
                })
                .cloned()
                .collect::<Vec<_>>()
        };
        let cmds = frame(&mut app, &ctx, raw());
        assert!(
            app.root.anim_started,
            "load animations start on the FIRST frame (no entrance gate)"
        );
        let wc = window_cmds(&cmds);
        assert!(
            wc.is_empty(),
            "R42: a pane host must issue no window commands; got {wc:?}"
        );
        // Even a direct window action is a no-op on the pane surface.
        app.apply_host_actions(
            &ctx,
            vec![cobolt_runtime::form_host::HostAction::SetWindowState {
                handle: cobolt_runtime::form_host::ROOT_HANDLE.to_string(),
                state: "Minimized".into(),
            }],
        );
        let cmds2 = frame(&mut app, &ctx, raw());
        let wc2 = window_cmds(&cmds2);
        assert!(
            wc2.is_empty(),
            "window commands are neutralised in Pane mode; got {wc2:?}"
        );
        println!(
            "049 AC8 (pane half) — entrance spec fade:3000 inert on Pane: \
             fx done at construction, animations on frame 1, 0 viewport \
             commands across 2 frames incl. an explicit SetWindowState"
        );
    }

    /// R7/R10 — while the entrance plays, the live UI (and the load-time
    /// control animations) wait; when it completes, both take over and the
    /// chrome is commanded back on.
    #[test]
    fn entrance_suppresses_live_ui_and_gates_load_animations() {
        let (mut app, _pipes) = host_with("fade:3000:linear", "none:0:linear", false);
        let ctx = egui::Context::default();

        assert!(!app.fx_entrance_done, "an active entrance starts pending");
        frame(&mut app, &ctx, raw());
        assert!(!app.fx_entrance_done, "3s entrance cannot finish in one frame");
        assert!(
            !app.root.anim_started,
            "load animations are gated behind the entrance (038 R8)"
        );

        // Force the playback clock past the end: the next frame completes the
        // entrance and restores the chrome.
        app.fx_entrance_start = Some(Instant::now() - Duration::from_millis(3200));
        let cmds = frame(&mut app, &ctx, raw());
        assert!(app.fx_entrance_done, "entrance completes past its duration");
        assert!(
            cmds.iter()
                .any(|c| matches!(c, egui::ViewportCommand::Decorations(true))),
            "the designed chrome comes back with the finished form; got {cmds:?}"
        );
        // …on the SAME frame, not the next one. The gate sits above the
        // playback block, so waiting for it would let the live UI paint one
        // frame of every load-animated control standing at its finished
        // position — a single-frame flash of exactly the picture the entrance
        // withheld.
        assert!(
            app.root.anim_started,
            "load animations must start on the frame the entrance completes"
        );
    }

    /// The other half of that rule: a control the entrance is holding back
    /// must not be painted into the entrance's face, while its neighbours are.
    #[test]
    fn the_entrance_face_leaves_out_load_animated_controls() {
        use cobolt_forms::model::{AnimKind, AnimTrigger, AnimationDef};

        let mut animated = cobolt_forms::Control::new(
            "Button-1",
            cobolt_forms::ControlType::Button,
            10,
            10,
        );
        let mut def = AnimationDef::new("intro");
        def.trigger = AnimTrigger::OnFormLoad;
        def.kind = AnimKind::FlyFromLeft;
        animated.add_animation(def);
        let plain =
            cobolt_forms::Control::new("Label-1", cobolt_forms::ControlType::Label, 10, 60);

        // The same filter `paint_face` applies, over both directions. (That
        // the painter passes `true` for an entrance and `false` for an exit is
        // fixed at its two call sites and checked by the compiler.)
        let controls = [animated, plain];
        let painted = |hide_load_animated: bool| -> Vec<&str> {
            controls
                .iter()
                .filter(|c| c.visible)
                .filter(|c| {
                    !(hide_load_animated && cobolt_forms::anim::has_load_animation(c))
                })
                .map(|c| c.id.as_str())
                .collect()
        };
        assert_eq!(
            painted(true),
            vec!["Label-1"],
            "the entrance must show the plain control and hold the flying one back"
        );
        assert_eq!(
            painted(false),
            vec!["Button-1", "Label-1"],
            "an exit holds nothing back — those animations played long ago"
        );
    }

    /// Without an entrance the gate is open from frame one — exactly the
    /// pre-038 behaviour.
    #[test]
    fn no_entrance_means_live_ui_from_the_first_frame() {
        let (mut app, _pipes) = host_with("none:0:linear", "none:0:linear", false);
        let ctx = egui::Context::default();
        assert!(app.fx_entrance_done);
        frame(&mut app, &ctx, raw());
        assert!(app.root.anim_started, "no entrance ⇒ load animations start at once");
    }

    /// R9 — restoring after a minimize replays the ENTRANCE visuals only: the
    /// playback re-arms, no form events fire, the load animations do not
    /// restart.
    #[test]
    fn restore_replays_the_entrance_without_events() {
        let (mut app, pipes) = host_with("fade:3000:linear", "none:0:linear", true);
        let ctx = egui::Context::default();
        // Settle the first entrance and the lifecycle one-shots.
        app.fx_entrance_done = true;
        app.root.anim_started = true;
        app.root.lifecycle_sent = true;
        app.minimized_actual = true; // was minimized …

        // … and this frame reports it restored.
        let mut input = raw();
        input.viewports.insert(
            egui::ViewportId::ROOT,
            egui::ViewportInfo {
                minimized: Some(false),
                ..Default::default()
            },
        );
        frame(&mut app, &ctx, input);

        assert!(!app.fx_entrance_done, "restore re-arms the entrance playback");
        let started = app
            .fx_entrance_start
            .expect("the same frame begins the fresh playback");
        assert!(
            started.elapsed() < Duration::from_millis(500),
            "the playback clock is fresh, not the first run's"
        );
        assert!(app.root.anim_started, "control animations do NOT replay");
        // 038 R9 narrowed, deliberately (2026-09-06). The rule is that a
        // restore must not look like the form LOADED AGAIN — it was written
        // when no window-state event existed, so "no form events" and "no
        // load lifecycle" were the same sentence. `onRestore` is the event
        // that exists to describe exactly this transition, and a handler bound
        // to it has to receive it, so the assertion now pins the part R9 is
        // actually about.
        let events = drain_events(&pipes);
        let names: Vec<&str> = events.iter().map(|(_, ev)| ev.as_str()).collect();
        for dead in ["onLoad", "onOpened", "onShow", "onActivate", "onActivated"] {
            assert!(
                !names.contains(&dead),
                "a restore replay must not re-run the load lifecycle, but sent \
                 {dead}: {names:?}"
            );
        }
        assert_eq!(
            names,
            ["onRestore"],
            "…and it sends the one event that names the transition"
        );
    }

    /// **A form's mouse events are its BACKGROUND** (operator ruling, "B").
    ///
    /// A click on a control belongs to that control and stops there; only bare
    /// form surface raises the form's own event. This is the whole ruling, and
    /// the half that is easy to get wrong is the negative one — so the same
    /// click is driven twice, once over a control and once beside it.
    #[test]
    fn a_form_click_is_the_background_not_a_control() {
        let (mut app, pipes) = host_with("none:0:linear", "none:0:linear", false);
        let ctx = egui::Context::default();
        app.fx_entrance_done = true;
        app.root.anim_started = true;
        app.root.lifecycle_sent = true;
        // A control occupying the top-left corner, as the engine painted it.
        app.root.last_control_rects.insert(
            "BTN".to_owned(),
            egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(100.0, 40.0)),
        );

        let names = |pipes: &_| -> Vec<String> {
            drain_events(pipes).into_iter().map(|(_, e)| e).collect()
        };
        let mut click_at = |app: &mut FormHost, at: egui::Pos2| {
            // Press and release in one frame: egui reports the click on the
            // release, which is the frame under test.
            let mut input = raw();
            input.viewports.insert(egui::ViewportId::ROOT, egui::ViewportInfo::default());
            input.events.push(egui::Event::PointerMoved(at));
            input.events.push(egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            });
            input.events.push(egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            });
            frame(app, &ctx, input);
        };

        // ON the control — the form must stay silent about the click.
        click_at(&mut app, egui::pos2(50.0, 20.0));
        let on_ctrl = names(&pipes);
        for ev in ["onClick", "onMouseDown", "onMouseUp", "onPointerDown"] {
            assert!(
                !on_ctrl.contains(&ev.to_owned()),
                "a click ON a control must not raise the form's {ev}: {on_ctrl:?}"
            );
        }

        // BESIDE it — the same gesture on bare form surface does raise them.
        click_at(&mut app, egui::pos2(400.0, 300.0));
        let on_bg = names(&pipes);
        for ev in ["onMouseDown", "onMouseUp", "onClick"] {
            assert!(
                on_bg.contains(&ev.to_owned()),
                "a click on the FORM must raise {ev}: {on_bg:?}"
            );
        }
        // …and the pointer aliases ride along, deliberately.
        assert!(on_bg.contains(&"onPointerDown".to_owned()));
        assert!(on_bg.contains(&"onPointerUp".to_owned()));
    }

    /// The finished form lands where the entrance played it, whichever way the
    /// platform adds a title bar.
    ///
    /// macOS grows the frame outward and leaves the content alone. Windows
    /// carves the title bar out of the window rect that already exists, so the
    /// client area shrinks from the top and its origin moves down — the effect
    /// runs across the strip that becomes the title bar, and the form then
    /// appears shifted down by that height (operator, 2026-09-09).
    ///
    /// Both are driven here as viewport readings, so neither depends on which
    /// machine the test runs on.
    #[test]
    fn the_form_stays_where_the_entrance_played_it_when_the_chrome_returns() {
        let played_in =
            egui::Rect::from_min_size(egui::pos2(100.0, 100.0), egui::vec2(800.0, 600.0));

        // Drive the entrance to its end, so the chrome goes back on and the
        // client rect it played in is recorded.
        let settle = |app: &mut FormHost, ctx: &egui::Context, inner: egui::Rect, outer: egui::Rect| {
            let mut input = raw();
            input.viewports.insert(
                egui::ViewportId::ROOT,
                egui::ViewportInfo {
                    inner_rect: Some(inner),
                    outer_rect: Some(outer),
                    ..Default::default()
                },
            );
            frame(app, ctx, input)
        };

        // ── The platform that carves the bar out of the window (Windows) ─────
        {
            let (mut app, _pipes) = host_with("fade:1:linear", "none:0:linear", false);
            let ctx = egui::Context::default();
            let outer = egui::Rect::from_min_size(played_in.min, played_in.size());
            // `fade` clamps to a 100 ms floor, so drive frames until it ends.
            for _ in 0..40 {
                settle(&mut app, &ctx, played_in, outer);
                if app.fx_entrance_done {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            assert!(app.fx_entrance_done, "the entrance never finished");
            assert!(
                app.fx_chrome_restore.is_some(),
                "the client rect the entrance played in must be recorded"
            );

            // The title bar takes 30 points off the top of the SAME window.
            let carved = egui::Rect::from_min_size(
                played_in.min + egui::vec2(0.0, 30.0),
                played_in.size() - egui::vec2(0.0, 30.0),
            );
            let cmds = settle(&mut app, &ctx, carved, outer);

            let sized = cmds.iter().any(
                |c| matches!(c, egui::ViewportCommand::InnerSize(v) if *v == played_in.size()),
            );
            let moved = cmds.iter().any(|c| matches!(
                c,
                egui::ViewportCommand::OuterPosition(p)
                    if (*p - (played_in.min - egui::vec2(0.0, 30.0))).length() < 0.5
            ));
            assert!(
                sized,
                "the client area must be given its designed size back: {cmds:?}"
            );
            assert!(
                moved,
                "the frame must be pulled up by the bar it grew, so the form \
                 lands where the effect left it: {cmds:?}"
            );
            assert!(app.fx_chrome_restore.is_none(), "and it asks only once");
        }

        // ── The platform that grows the frame outward (macOS) ────────────────
        {
            let (mut app, _pipes) = host_with("fade:1:linear", "none:0:linear", false);
            let ctx = egui::Context::default();
            let outer =
                egui::Rect::from_min_size(played_in.min - egui::vec2(0.0, 30.0), played_in.size());
            for _ in 0..40 {
                settle(&mut app, &ctx, played_in, outer);
                if app.fx_entrance_done {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            assert!(app.fx_entrance_done, "the entrance never finished");

            // The content did not move, so nothing may be sent about it.
            let cmds = settle(&mut app, &ctx, played_in, outer);
            assert!(
                !cmds.iter().any(|c| matches!(
                    c,
                    egui::ViewportCommand::InnerSize(_) | egui::ViewportCommand::OuterPosition(_)
                )),
                "the content already sits where it was — correcting it would be \
                 the bug, not the fix: {cmds:?}"
            );
        }
    }

    /// **Focus, theme, DPI, geometry, clipboard, drag and scroll all reach the
    /// form.**
    ///
    /// Twenty-one events that were designable and never sent. Each is driven
    /// here through the real input path, and the steady state is checked too:
    /// these are edges and gestures, not per-frame chatter.
    /// A shell's main form hears its window being resized — as a form in a
    /// window of its own does — with the size of what it lays out in: the
    /// ContentPane (plus the rail's designed column). They were behind a
    /// Window-only guard, and PowerChat's CHAT-FORM `onResize` never ran
    /// (operator, 2026-09-29). And a rail toggle, which resizes the WINDOW by
    /// the rail's width so the pane keeps its own, is not a resize of the form
    /// at all: reported as one, a collapse shrank the controls a handler laid
    /// out from `Width` (operator, same day).
    #[test]
    fn a_shell_form_hears_its_pane_resize_and_not_a_rail_toggle() {
        let (mut app, pipes) = host_with_surface("none:0:linear", "none:0:linear", false, Surface::Pane);
        let ctx = egui::Context::default();
        app.fx_entrance_done = true;
        app.root.anim_started = true;
        app.root.lifecycle_sent = true;
        // `pane` is what the shell leaves the form; `window` the OS window.
        let mut step = |app: &mut FormHost, pane_w: f32, window_w: f32| {
            let mut input = raw();
            input.screen_rect = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(pane_w, 600.0)));
            input.viewports.insert(
                egui::ViewportId::ROOT,
                egui::ViewportInfo {
                    focused: Some(true),
                    inner_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(window_w, 600.0))),
                    ..Default::default()
                },
            );
            frame(app, &ctx, input);
        };
        let names = |pipes: &Pipes| -> Vec<String> {
            drain_events(pipes).into_iter().map(|(_, e)| e).filter(|e| e.starts_with("onResiz")).collect()
        };
        let sizes = |pipes: &Pipes| -> Vec<(String, String)> {
            pipes
                ._input_rx
                .try_iter()
                .filter(|u| u.prop == "Width" || u.prop == "Height")
                .map(|u| (u.prop, u.value))
                .collect()
        };
        step(&mut app, 800.0, 1064.0);
        let _ = (names(&pipes), sizes(&pipes));

        // The rail collapses: the window gives back its width, the pane stays.
        step(&mut app, 800.0, 864.0);
        step(&mut app, 800.0, 864.0);
        assert!(names(&pipes).is_empty(), "a rail toggle is not a resize of the form");
        assert!(sizes(&pipes).is_empty(), "…and changes no size the form reads");

        // The operator drags the window wider: the pane grows with it.
        step(&mut app, 900.0, 964.0);
        assert_eq!(names(&pipes), ["onResizing"]);
        step(&mut app, 900.0, 964.0);
        assert_eq!(names(&pipes), ["onResize"]);
        let last = sizes(&pipes);
        assert_eq!(
            &last[last.len() - 2..],
            &[("Width".to_string(), "900".to_string()), ("Height".to_string(), "600".to_string())],
            "the handler reads the pane's size"
        );

        // A pane that wobbles for a frame and comes back settles on no change.
        step(&mut app, 964.0, 964.0);
        step(&mut app, 900.0, 964.0);
        step(&mut app, 900.0, 964.0);
        assert_eq!(names(&pipes), ["onResizing", "onResizing"], "no onResize for a size it already reported");
    }

    #[test]
    fn the_input_and_environment_events_reach_the_form() {
        let (mut app, pipes) = host_with("none:0:linear", "none:0:linear", false);
        let ctx = egui::Context::default();
        app.fx_entrance_done = true;
        app.root.anim_started = true;
        app.root.lifecycle_sent = true;

        let names = |pipes: &_| -> Vec<String> {
            drain_events(pipes).into_iter().map(|(_, e)| e).collect()
        };
        // A settled baseline, so the first frame's readings are not "changes".
        let base = || egui::ViewportInfo {
            focused: Some(true),
            native_pixels_per_point: Some(2.0),
            inner_rect: Some(egui::Rect::from_min_size(
                egui::pos2(0.0, 0.0),
                egui::vec2(800.0, 600.0),
            )),
            outer_rect: Some(egui::Rect::from_min_size(
                egui::pos2(10.0, 10.0),
                egui::vec2(800.0, 600.0),
            )),
            ..Default::default()
        };
        let mut step = |app: &mut FormHost, info: egui::ViewportInfo, mutate: &dyn Fn(&mut egui::RawInput)| {
            let mut input = raw();
            input.viewports.insert(egui::ViewportId::ROOT, info);
            mutate(&mut input);
            frame(app, &ctx, input);
        };
        let nothing = |_: &mut egui::RawInput| {};

        step(&mut app, base(), &nothing);
        let _ = names(&pipes); // discard the settling frame

        // Focus.
        let mut unfocused = base();
        unfocused.focused = Some(false);
        step(&mut app, unfocused.clone(), &nothing);
        assert_eq!(names(&pipes), ["onLostFocus"]);
        step(&mut app, base(), &nothing);
        assert_eq!(names(&pipes), ["onGotFocus"]);

        // DPI — a drag onto a display with another scale factor.
        let mut hidpi = base();
        hidpi.native_pixels_per_point = Some(1.0);
        step(&mut app, hidpi, &nothing);
        assert_eq!(names(&pipes), ["onDpiChanged"]);

        // Geometry: the progressive name repeats, the base name settles it.
        let mut bigger = base();
        bigger.native_pixels_per_point = Some(1.0);
        bigger.inner_rect = Some(egui::Rect::from_min_size(
            egui::pos2(0.0, 0.0),
            egui::vec2(900.0, 600.0),
        ));
        // Each carries the new size on the form object, sent before the
        // event, so the handler reads it.
        let sizes = |pipes: &Pipes| -> Vec<(String, String)> {
            pipes
                ._input_rx
                .try_iter()
                .filter(|u| u.prop == "Width" || u.prop == "Height")
                .map(|u| (u.prop, u.value))
                .collect()
        };
        let _ = sizes(&pipes);
        let resized = vec![("Width".to_string(), "900".to_string()), ("Height".to_string(), "600".to_string())];
        step(&mut app, bigger.clone(), &nothing);
        assert_eq!(names(&pipes), ["onResizing"], "while the size is changing");
        assert_eq!(sizes(&pipes), resized, "the handler reads the size it is resizing to");
        step(&mut app, bigger.clone(), &nothing);
        assert_eq!(names(&pipes), ["onResize"], "…and once when it settles");
        assert_eq!(sizes(&pipes), resized, "…and the size it settled at");
        step(&mut app, bigger.clone(), &nothing);
        assert!(names(&pipes).is_empty(), "a settled window repeats nothing");

        // Clipboard.
        step(&mut app, bigger.clone(), &|i| {
            i.events.push(egui::Event::Copy);
            i.events.push(egui::Event::Paste("x".into()));
        });
        assert_eq!(names(&pipes), ["onCopy", "onPaste"]);

        // Drag and drop.
        step(&mut app, bigger.clone(), &|i| {
            i.hovered_files.push(egui::HoveredFile::default());
        });
        assert_eq!(names(&pipes), ["onDragEnter", "onDragOver"]);
        step(&mut app, bigger.clone(), &nothing);
        assert_eq!(names(&pipes), ["onDragLeave"], "the pointer left without dropping");

        // Scrolling brackets its gesture.
        step(&mut app, bigger.clone(), &|i| {
            i.events.push(egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0.0, -40.0),
                phase: egui::TouchPhase::Move,
                modifiers: egui::Modifiers::NONE,
            });
        });
        let scrolled = names(&pipes);
        assert!(scrolled.starts_with(&["onScrollStart".to_owned(), "onScroll".to_owned()]),
            "a gesture opens with start then scroll: {scrolled:?}");
        assert!(scrolled.contains(&"onVerticalScroll".to_owned()));
        assert!(!scrolled.contains(&"onHorizontalScroll".to_owned()), "no x movement");
    }

    /// **The window-state events fire on the OBSERVED edge, once each.**
    ///
    /// `onMinimize`, `onMaximize`, `onRestore`, `onFullscreen` and
    /// `onExitFullscreen` were designable and never sent — a handler bound to
    /// any of them was dead (operator, 2026-09-06). They ride the same
    /// ViewportInfo the entrance replay already watched.
    #[test]
    fn the_window_state_events_fire_on_the_observed_edge() {
        let (mut app, pipes) = host_with("none:0:linear", "none:0:linear", false);
        let ctx = egui::Context::default();
        app.fx_entrance_done = true;
        app.root.anim_started = true;
        app.root.lifecycle_sent = true;

        let step = |app: &mut FormHost, ctx: &egui::Context, info: egui::ViewportInfo| {
            let mut input = raw();
            input.viewports.insert(egui::ViewportId::ROOT, info);
            frame(app, ctx, input);
        };
        let names = |pipes: &_| -> Vec<String> {
            drain_events(pipes).into_iter().map(|(_, e)| e).collect()
        };

        // Maximize, then back.
        step(&mut app, &ctx, egui::ViewportInfo { maximized: Some(true), ..Default::default() });
        assert_eq!(names(&pipes), ["onMaximize"], "the maximize edge");
        step(&mut app, &ctx, egui::ViewportInfo { maximized: Some(false), ..Default::default() });
        assert_eq!(names(&pipes), ["onRestore"], "…and leaving it is a restore");

        // Fullscreen carries BOTH the directional event and the historical
        // `onFullScreenChanged`, which existing forms may already bind.
        step(&mut app, &ctx, egui::ViewportInfo { fullscreen: Some(true), ..Default::default() });
        assert_eq!(
            names(&pipes),
            ["onFullScreenChanged", "onFullscreen"],
            "entering fullscreen"
        );
        step(&mut app, &ctx, egui::ViewportInfo { fullscreen: Some(false), ..Default::default() });
        assert_eq!(
            names(&pipes),
            ["onFullScreenChanged", "onExitFullscreen"],
            "and leaving it"
        );

        // Minimize hides the form; it is still alive, so this is not teardown.
        step(&mut app, &ctx, egui::ViewportInfo { minimized: Some(true), ..Default::default() });
        assert_eq!(names(&pipes), ["onMinimize", "onHide"], "minimize also hides");

        // A steady state repeats nothing — these are EDGES.
        step(&mut app, &ctx, egui::ViewportInfo { minimized: Some(true), ..Default::default() });
        assert!(
            names(&pipes).is_empty(),
            "an unchanged window state must send nothing"
        );
    }

    // ── Group 4: lifecycle (R12–R15) ─────────────────────────────────────────

    /// R13 — onShow/onActivate fire exactly once, after the input warm-up.
    #[test]
    fn lifecycle_fires_once_after_warmup() {
        let (mut app, pipes) = host_with("none:0:linear", "none:0:linear", false);
        let ctx = egui::Context::default();

        frame(&mut app, &ctx, raw());
        assert!(
            drain_events(&pipes).is_empty(),
            "no lifecycle before the 450 ms warm-up"
        );

        app.root.start = Instant::now() - Duration::from_millis(600);
        frame(&mut app, &ctx, raw());
        let evs = drain_events(&pipes);
        assert_eq!(
            evs,
            vec![
                ("PARITY-FORM".to_owned(), "onShow".to_owned()),
                ("PARITY-FORM".to_owned(), "onActivate".to_owned()),
            ],
            "onShow then onActivate, addressed to the form"
        );

        frame(&mut app, &ctx, raw());
        assert!(drain_events(&pipes).is_empty(), "one-shot: no repeats");
    }

    /// R15 — the program ending closes the window: immediately with no exit
    /// effect…
    #[test]
    fn program_end_closes_the_window() {
        let (mut app, pipes) = host_with("none:0:linear", "none:0:linear", false);
        let ctx = egui::Context::default();
        pipes.finished.store(true, Ordering::Relaxed);
        let cmds = frame(&mut app, &ctx, raw());
        assert!(
            cmds.iter().any(|c| matches!(c, egui::ViewportCommand::Close)),
            "STOP RUN closes the window; got {cmds:?}"
        );
    }

    /// …and through the exit effect when one is configured (R15 × R10), with
    /// the chrome stepping aside and exactly ONE quit at the real close (R13).
    #[test]
    fn program_end_plays_the_exit_then_quits_once() {
        let (mut app, pipes) = host_with("none:0:linear", "fade:3000:linear", false);
        let ctx = egui::Context::default();
        pipes.finished.store(true, Ordering::Relaxed);

        let cmds = frame(&mut app, &ctx, raw());
        assert!(app.fx_exit_start.is_some(), "program end arms the exit effect");
        assert!(
            !cmds.iter().any(|c| matches!(c, egui::ViewportCommand::Close)),
            "no close while the exit is still receding; got {cmds:?}"
        );
        assert!(
            cmds.iter()
                .any(|c| matches!(c, egui::ViewportCommand::Decorations(false))),
            "the chrome steps aside for the exit; got {cmds:?}"
        );
        assert!(drain_events(&pipes).is_empty(), "quit only at the real close");

        // Past the end of the playback: the REAL close happens, once.
        app.fx_exit_start = Some(Instant::now() - Duration::from_millis(3200));
        let cmds = frame(&mut app, &ctx, raw());
        assert!(
            cmds.iter().any(|c| matches!(c, egui::ViewportCommand::Close)),
            "the exit's end performs the actual close; got {cmds:?}"
        );
        let evs = drain_events(&pipes);
        assert_eq!(
            evs,
            vec![("__QUIT__".to_owned(), "Quit".to_owned())],
            "exactly one quit → exactly one onClose downstream"
        );

        frame(&mut app, &ctx, raw());
        assert!(drain_events(&pipes).is_empty(), "quit is one-shot");
    }

    // ── Group 5: window assembly (R7, R17) ───────────────────────────────────

    /// R17 — the designed title wins; the fallback covers only blank designs.
    #[test]
    fn window_title_rule() {
        assert_eq!(window_title("Designed", "App v1".into()), "Designed");
        assert_eq!(window_title("  ", "App v1".into()), "App v1");
        assert_eq!(window_title("", String::new()), "");
    }

    /// R7 — the surface class per entrance effect: face-movers and MatrixRain
    /// get the see-through window, masked reveals keep an opaque one, and no
    /// entrance means the plain designed window.
    #[test]
    fn fx_window_flags_match_the_effect_class() {
        let flags = |s: &str| fx_window_flags(&FxSpec::parse(s));
        assert_eq!(flags("none:600:ease-out"), (false, false));
        assert_eq!(flags("fade:600:ease-out"), (true, true));
        assert_eq!(flags("matrix-rain:1500:linear"), (true, true));
        assert_eq!(flags("zoom:600:ease-out"), (true, true));
        // Masked reveals paint covers — nothing transparent can undo them.
        assert_eq!(flags("radar-wipe:600:ease-out"), (true, false));
        assert_eq!(flags("iris-wipe:600:ease-out"), (true, false));
        assert_eq!(flags("blinds:600:ease-out"), (true, false));
        assert_eq!(flags("checkerboard:600:ease-out"), (true, false));
    }

    // ── Group 1/2/6/7 pointers + the honest report ───────────────────────────

    /// The quantified summary the operator's test-reporting rule requires:
    /// what ran, where the rest of the suite lives, and what is deliberately
    /// left to the manual pass.
    #[test]
    fn zz_parity_report() {
        println!("┌─────────────────────────────────────────────────────────────┐");
        println!("│ spec 042 parity suite — one host, every surface             │");
        println!("├─────────────────────────────────────────────────────────────┤");
        println!("│ group 1 state       4 tests  src/state.rs (incl. 1.60.33    │");
        println!("│                              dedupe — delete it, they fail) │");
        println!("│ group 2 seeding     2 tests  src/seeding.rs                 │");
        println!("│ group 3 fx gating   3 tests  this module                    │");
        println!("│ group 4 lifecycle   3 tests  this module                    │");
        println!("│ group 5 window      2 tests  this module                    │");
        println!("│ group 7 diagnostics 2 tests  src/diagnostics.rs             │");
        println!("│ per-host glue: cobolt-cli tests (4) + cobolt-compiler       │");
        println!("│ template-content tests + real `cargo build` compile gate    │");
        println!("├─────────────────────────────────────────────────────────────┤");
        println!("│ NOT covered here (operator's manual pass / by design):      │");
        println!("│  · real OS windowing: actual transparency, decorations,     │");
        println!("│    start position on a monitor, minimize/restore signals    │");
        println!("│  · close-veto via the OS close button (close_requested is   │");
        println!("│    driven by winit; the veto machine is covered by          │");
        println!("│    cobolt-runtime's FormSupervisor tests)                   │");
        println!("│  · group 6 I/O & pacing under a live render (timer          │");
        println!("│    coalescing, DISPLAY flush syscall) — logic moved         │");
        println!("│    verbatim from the proven run-form host                   │");
        println!("└─────────────────────────────────────────────────────────────┘");
    }

    // ── Spec 056 §4.16 — the host golden of the example corpus (R79; AC2) ──
    //
    // Every `.cfrm` of PowerDemo3 and PowerChat as the ROOT of a window, as the
    // root of a SideMenu shell (Pane mode, forms with a SideMenu only) and as a
    // ContentPane OCCUPANT of a plain shell, at 0.75×, 1× and 1.5× its designed
    // size: what `last_control_rects` records, plus each control's font size.
    // It is the only coverage of `stretch_window_bars`, the Pane construction
    // shift and the running shell's `rail_view`. Captured before the first
    // responsive-layout change; `COBOLT_WRITE_GOLDEN=1` rewrites it.

    fn corpus_repo() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn corpus_forms(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
        fn walk(d: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
            let Ok(rd) = std::fs::read_dir(d) else { return };
            for e in rd.flatten() {
                let p = e.path();
                if p.is_dir() {
                    walk(&p, out);
                } else if p.extension().and_then(|e| e.to_str()) == Some("cfrm") {
                    out.push(p);
                }
            }
        }
        let mut out = Vec::new();
        walk(dir, &mut out);
        out.sort();
        out
    }

    /// `[forms] theme` of a project manifest, read by line (the host crate has
    /// no TOML parser, and this is the one key needed).
    fn corpus_theme_default(project_dir: &std::path::Path, project: &str) -> Option<String> {
        let text = std::fs::read_to_string(project_dir.join(format!("{project}.project.toml"))).ok()?;
        let mut in_forms = false;
        for line in text.lines() {
            let t = line.trim();
            if t.starts_with('[') {
                in_forms = t == "[forms]";
                continue;
            }
            if in_forms {
                if let Some(v) = t.strip_prefix("theme") {
                    let v = v.trim_start().strip_prefix('=')?.trim().trim_matches('"');
                    return Some(v.to_owned());
                }
            }
        }
        None
    }

    fn corpus_theme(
        form: &cobolt_forms::Form,
        default: Option<&str>,
    ) -> (
        Option<Arc<cobolt_forms::theme_pack::ThemePack>>,
        Arc<dyn cobolt_forms::surface_theme::SurfaceTheme>,
    ) {
        let id = cobolt_forms::theme::resolve_theme_id(form.theme.as_deref(), default);
        let pack = if cobolt_forms::theme::ThemeCatalog::procedural_ids().contains(&id.as_str()) {
            None
        } else {
            cobolt_forms::theme_pack::discover_packs(&corpus_repo().join("assets/themes"))
                .into_iter()
                .find(|p| p.id == id)
                .map(Arc::new)
        };
        let st = match pack.as_ref() {
            Some(p) => cobolt_forms::surface_theme::for_pack(p.manifest.self_contained),
            None => cobolt_forms::surface_theme::for_theme_id(&id),
        };
        (pack, st)
    }

    fn corpus_program() -> cobolt_ast::program::Program {
        let src = "IDENTIFICATION DIVISION.\nPROGRAM-ID. OCC.\nPROCEDURE DIVISION.\n    STOP RUN.\n";
        cobolt_parser::parse(cobolt_lexer::tokenize(src, cobolt_lexer::SourceFormat::Free))
            .program
            .expect("parses")
    }

    /// A host for `form` exactly as the glue builds one: flattened controls,
    /// state seeded from them, the resolved theme, no window effects.
    fn corpus_host(
        form: cobolt_forms::Form,
        surface: Surface,
        theme_default: Option<String>,
        form_source: Option<FormSource>,
    ) -> (FormHost, cobolt_forms::Form, Pipes) {
        let mut flat = Vec::new();
        crate::flatten_controls(&form.controls, &mut flat);
        let state: HashMap<String, CtrlState> =
            flat.iter().map(|c| (c.id.clone(), CtrlState::from_control(c))).collect();
        let (theme_pack, surface_theme) = corpus_theme(&form, theme_default.as_deref());
        let (ev_tx, ev_rx) = mpsc::channel();
        let (input_tx, _input_rx) = mpsc::channel();
        let (_state_tx, state_rx) = mpsc::channel();
        let (_display_tx, display_rx) = mpsc::channel();
        let (form_req_tx, form_req_rx) = mpsc::channel();
        let (closed_tx, _closed_rx) = mpsc::channel();
        let finished = Arc::new(AtomicBool::new(false));
        let child_theme: ChildThemeSource =
            Box::new(move |child: &cobolt_forms::Form| corpus_theme(child, theme_default.as_deref()));
        let (mut app, form) = FormHost::new(FormHostConfig {
            form,
            flat,
            state,
            ev_tx,
            input_tx,
            state_rx,
            display_rx,
            pending: Arc::new(AtomicUsize::new(0)),
            finished: Arc::clone(&finished),
            form_req_rx,
            closed_tx,
            form_req_tx: form_req_tx.clone(),
            form_source,
            child_theme: Some(child_theme),
            child_interpreter_setup: None,
            indexed_engine: Default::default(),
            shared_rust_bridge: None,
            fx_entrance: FxSpec::default(),
            fx_exit: FxSpec::default(),
            fx_restore: false,
            theme_pack,
            surface_theme,
            icon_path: None,
            title_fallback: String::new(),
            hooks: Box::new(NoHooks),
            surface,
        });
        app.fx_entrance_done = true;
        app.root.anim_started = true;
        app.root.lifecycle_sent = true;
        (
            app,
            form,
            Pipes {
                ev_rx,
                _input_rx,
                _state_tx,
                _display_tx,
                finished,
                _form_req_tx: form_req_tx,
                _closed_rx,
            },
        )
    }

    /// The shell `run_shell` builds around a Pane-mode root (`shell.rs`).
    fn corpus_shell(form: &cobolt_forms::Form, forms_dir: &std::path::Path) -> crate::shell::Shell {
        let mut shell = crate::shell::Shell::default();
        shell.menu_background = form.menu_pane_background.clone();
        let side = form.side_menu_control_id().and_then(|id| form.find_control(&id).cloned());
        if let Some(side) = side.as_ref() {
            shell.collapsed = side.side_menu_collapsed();
            shell.full_height = side.side_menu_full_height();
            shell.icon_effect = side
                .get_prop("IconEffect")
                .map(|v| v.as_str().to_owned())
                .unwrap_or_else(|| "None".to_owned());
            shell.breadcrumb_height = cobolt_forms::breadcrumb::height_of(side);
            shell.breadcrumb_bg = side
                .breadcrumb_background()
                .map(|hex| cobolt_forms::paint::parse_color(&hex));
            if side.rect.w > 0 {
                shell.menu_open_width = side.rect.w as f32;
            }
            shell.menu_collapsed_width = side.side_menu_collapsed_width();
            let yaml = cobolt_forms::menu::menu_yaml_path(forms_dir, &side.id);
            if let Ok(def) = cobolt_forms::menu::load_menu(&yaml) {
                shell.mount_root_menu(&form.name, def);
            }
        }
        shell.side_ctrl = side;
        shell.form_backdrop = Some(cobolt_forms::render::backdrop_color(
            &form.background_color,
            form.transparency,
        ));
        shell
    }

    fn corpus_input(size: egui::Vec2) -> egui::RawInput {
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
            max_texture_side: Some(8192),
            time: Some(1.0),
            ..Default::default()
        }
    }

    /// Rects + font sizes of one body, sorted by control id.
    fn corpus_rows(body: &FormBody, rects: &HashMap<String, egui::Rect>) -> Vec<String> {
        fn q(v: f32) -> f32 {
            (v * 4.0).round() / 4.0
        }
        let mut ids: Vec<&cobolt_forms::Control> = body.controls.iter().collect();
        ids.sort_by(|a, b| a.id.cmp(&b.id));
        ids.iter()
            .map(|c| {
                let rect = rects
                    .get(&c.id)
                    .map(|r| format!("[{} {} {} {}]", q(r.min.x), q(r.min.y), q(r.max.x), q(r.max.y)))
                    .unwrap_or_else(|| "-".into());
                let live = match body.state.get(&c.id) {
                    Some(st) => cobolt_forms::render::merge_props(c, st.props.iter()),
                    None => (*c).clone(),
                };
                format!("{} rect={rect} font={}", c.id, q(cobolt_forms::paint::ctrl_font_size(&live)))
            })
            .collect()
    }

    const CORPUS_FRAMES: usize = 3;

    /// `form` as the ContentPane occupant of a plain shell `size` large: its
    /// rows and the pane it was given.
    fn corpus_occupant(
        form: &cobolt_forms::Form,
        theme_default: &Option<String>,
        size: egui::Vec2,
    ) -> (Vec<String>, Option<egui::Rect>) {
        let occupant = form.clone();
        let key = occupant.name.trim().to_ascii_uppercase();
        let source: FormSource = Box::new(move |_id: &str| Ok((occupant.clone(), corpus_program())));
        let shell_form = cobolt_forms::Form::new("CORPUS-SHELL", "Shell", size.x as u32, size.y as u32);
        let (mut app, _f, _pipes) = corpus_host(shell_form, Surface::Pane, theme_default.clone(), Some(source));
        app.ensure_occupant(&key).expect("the occupant builds");
        app.show_occupant(Some(&key));
        let mut shell = crate::shell::Shell::default();
        let ctx = egui::Context::default();
        ctx.set_fonts(cobolt_forms::fonts::base_font_definitions());
        for _ in 0..CORPUS_FRAMES {
            let mut full = ctx.run_ui(corpus_input(size), |ui| {
                shell.show_with_host(ui, |_ui| {}, &mut app);
            });
            full.textures_delta.clear();
        }
        let occ = &app.pane.occupants[&key];
        (corpus_rows(&occ.body, app.last_control_rects()), app.last_occupant_rect())
    }

    /// The host surfaces of the corpus for one form at one window size, each
    /// with its rows: root Window, root Pane (SideMenu forms only) and the
    /// ContentPane occupant of a plain shell (`occupant_pane_designed`: that
    /// shell grown so its pane is the form's designed size).
    fn corpus_surfaces(
        form: &cobolt_forms::Form,
        forms_dir: &std::path::Path,
        theme_default: &Option<String>,
        size: egui::Vec2,
        occupant_pane_designed: bool,
    ) -> Vec<(&'static str, Vec<String>)> {
        let mut out = Vec::new();
        // Root Window.
        {
            let (mut app, _form, _pipes) =
                corpus_host(form.clone(), Surface::Window, theme_default.clone(), None);
            let ctx = egui::Context::default();
            ctx.set_fonts(cobolt_forms::fonts::base_font_definitions());
            for _ in 0..CORPUS_FRAMES {
                frame(&mut app, &ctx, corpus_input(size));
            }
            out.push(("Window", corpus_rows(&app.root, app.last_control_rects())));
        }
        // Root Pane — a SideMenu shell.
        if form.has_side_menu() {
            let (mut app, root_form, _pipes) =
                corpus_host(form.clone(), Surface::Pane, theme_default.clone(), None);
            let mut shell = corpus_shell(&root_form, forms_dir);
            let ctx = egui::Context::default();
            ctx.set_fonts(cobolt_forms::fonts::base_font_definitions());
            for _ in 0..CORPUS_FRAMES {
                let mut full = ctx.run_ui(corpus_input(size), |ui| {
                    shell.show_with_host(ui, |_ui| {}, &mut app);
                });
                full.textures_delta.clear();
            }
            out.push(("Pane", corpus_rows(&app.root, app.last_control_rects())));
        }
        // ContentPane occupant of a plain shell. For R81 the shell is grown
        // so the PANE, the occupant's surface, is the designed size.
        let (mut rows, pane) = corpus_occupant(form, theme_default, size);
        if occupant_pane_designed {
            if let Some(pane) = pane {
                let grown = size + (egui::vec2(form.width as f32, form.height as f32) - pane.size());
                rows = corpus_occupant(form, theme_default, grown).0;
            }
        }
        out.push(("Occupant", rows));
        out
    }

    #[test]
    fn corpus_golden() {
        let started = Instant::now();
        let write = std::env::var("COBOLT_WRITE_GOLDEN").is_ok();
        let golden_root =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/goldens/056_corpus");
        let factors = [(0.75f32, "0.75x"), (1.0, "1x"), (1.5, "1.5x")];
        let (mut forms, mut rows, mut renders, mut panes, mut written) = (0usize, 0usize, 0usize, 0usize, 0usize);
        let mut differences = Vec::new();
        let (mut responsive_checked, mut responsive_diffs) = (0usize, Vec::<String>::new());
        for project in ["PowerDemo3", "PowerChat"] {
            let dir = corpus_repo().join("examples").join(project);
            cobolt_forms::assets::set_base(&dir);
            let theme_default = corpus_theme_default(&dir, project);
            let forms_dir = dir.join("forms");
            for path in corpus_forms(&forms_dir) {
                let rel = path.strip_prefix(&forms_dir).unwrap().display().to_string();
                let form = cobolt_forms::load_form(&path)
                    .unwrap_or_else(|e| panic!("{} must parse: {e}", path.display()));
                forms += 1;
                let mut text = format!("# {project}/{rel}\n");
                for (f, fname) in factors {
                    let size = egui::vec2(
                        (form.width as f32 * f).round().max(64.0),
                        (form.height as f32 * f).round().max(64.0),
                    );
                    for (surface, r) in corpus_surfaces(&form, &forms_dir, &theme_default, size, false) {
                        rows += r.len();
                        renders += 1;
                        if surface == "Pane" {
                            panes += 1;
                        }
                        text.push_str(&format!("## {surface} {fname} {}x{}\n{}\n", size.x, size.y, r.join("\n")));
                    }
                }
                // R81 / AC38 — the same form with only `responsive="true"` lays
                // out at its designed size exactly as the golden does, on every
                // host surface. A form that already lays itself out is skipped:
                // its golden occupant was laid out for the pane, which the R81
                // copy grows to the designed size, so the two cannot agree.
                if !form.lays_out() {
                    let mut copy = form.clone();
                    copy.responsive = true;
                    let size = egui::vec2(
                        (form.width as f32).round().max(64.0),
                        (form.height as f32).round().max(64.0),
                    );
                    for (surface, r) in corpus_surfaces(&copy, &forms_dir, &theme_default, size, true) {
                        let header = format!("## {surface} 1x {}x{}\n", size.x, size.y);
                        let plain = text
                            .split(&header)
                            .nth(1)
                            .and_then(|rest| rest.split("\n## ").next())
                            .unwrap_or_default()
                            .trim_end()
                            .to_owned();
                        let got = r.join("\n");
                        responsive_checked += 1;
                        if plain != got {
                            let first = plain.lines().zip(got.lines()).find(|(a, b)| a != b);
                            responsive_diffs.push(format!("{project}/{rel} {surface}: {first:?}"));
                        }
                    }
                }
                let file = golden_root
                    .join(project)
                    .join(format!("{}.txt", rel.replace(['/', '\\'], "__")));
                if write {
                    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
                    std::fs::write(&file, &text).unwrap();
                    written += 1;
                    continue;
                }
                let expected = std::fs::read_to_string(&file).unwrap_or_else(|_| {
                    panic!("golden {} missing — capture it with COBOLT_WRITE_GOLDEN=1", file.display())
                });
                if expected != text {
                    let e: Vec<_> = expected.lines().collect();
                    let g: Vec<_> = text.lines().collect();
                    let differing = e.iter().zip(g.iter()).filter(|(a, b)| a != b).count()
                        + e.len().abs_diff(g.len());
                    let first = e.iter().zip(g.iter()).position(|(a, b)| a != b).unwrap_or(e.len().min(g.len()));
                    differences.push(format!(
                        "{project}/{rel}: {differing} line(s) differ; first at line {}:\n    expected: {}\n    got:      {}",
                        first + 1,
                        e.get(first).unwrap_or(&"<end>"),
                        g.get(first).unwrap_or(&"<end>")
                    ));
                }
            }
        }
        println!("── 056 host corpus golden ────────────────────────────────");
        println!("  projects   : PowerDemo3, PowerChat");
        println!("  forms      : {forms} ({panes} SideMenu shell renders)");
        println!("  sizes      : 0.75x, 1x, 1.5x");
        println!("  surfaces   : root Window, root Pane (SideMenu forms), ContentPane occupant");
        println!("  renders    : {renders} ({CORPUS_FRAMES} frames each, last recorded)");
        println!("  rects+fonts: {rows} control rows compared");
        if write {
            println!("  WROTE      : {written} golden files under {}", golden_root.display());
        } else {
            println!("  differences: {} form(s)", differences.len());
        }
        println!(
            "  responsive : {responsive_checked} designed-size renders with responsive=\"true\", {} differing from the plain form",
            responsive_diffs.len()
        );
        println!("  elapsed    : {:.1} s", started.elapsed().as_secs_f32());
        assert!(
            differences.is_empty(),
            "the example corpus no longer lays out as its host golden:\n{}",
            differences.join("\n")
        );
        assert!(
            responsive_diffs.is_empty(),
            "R81 — turning Responsive design on moved something at the designed size:\n{}",
            responsive_diffs.join("\n")
        );
    }


    /// Spec 056 T7.2 (AC43, R38) — a program's geometry write on a responsive
    /// form is an on-screen value that composes with anchoring.
    /// A 400 × 300 form, BTN at (300, 20, 80, 30) anchored `Top,Right`, in a
    /// 600 × 400 window: on screen at x 500. `ADD 10 TO BTN::X` twice writes
    /// 510 then 520 → on screen 510, 520. Widen the window to 700: the button
    /// keeps its new distance from the right edge (0 px) → x 620.
    /// And the responsive copy of `a_moved_container_carries_its_contents`:
    /// PNL at (40, 150, 200, 100) anchored `Top,Left` holding LBL at (50, 160);
    /// `SET PNL::Y TO 110` moves both up 40 on screen.
    #[test]
    fn geometry_writes_compose_with_the_layout_056() {
        let mut f = cobolt_forms::Form::new("GEO-FORM", "Geometry", 400, 300);
        f.responsive = true;
        let mut b = cobolt_forms::Control::new("BTN", cobolt_forms::ControlType::Button, 0, 0);
        b.rect = cobolt_forms::model::Rect::new(300, 20, 80, 30);
        b.set_prop("Anchor", cobolt_forms::PropValue::String("Top,Right".into()));
        let mut p = cobolt_forms::Control::new("PNL", cobolt_forms::ControlType::Panel, 0, 0);
        p.rect = cobolt_forms::model::Rect::new(40, 150, 200, 100);
        let mut l = cobolt_forms::Control::new("LBL", cobolt_forms::ControlType::Label, 0, 0);
        l.rect = cobolt_forms::model::Rect::new(50, 160, 80, 20);
        l.parent = Some("PNL".into());
        f.controls.extend([b, p, l]);
        let (mut app, _f, _p) = corpus_host(f, Surface::Window, None, None);
        let ctx = egui::Context::default();
        let mut run = |app: &mut FormHost, w: f32, h: f32| {
            for _ in 0..CORPUS_FRAMES {
                frame(app, &ctx, corpus_input(egui::vec2(w, h)));
            }
            app.last_control_rects().clone()
        };
        let r = run(&mut app, 600.0, 400.0);
        assert_eq!(r["BTN"].min.x, 500.0);
        for to in ["510", "520"] {
            app.root.apply_interpreter_update(StateUpdate::new("BTN", "X", to), false);
            let r = run(&mut app, 600.0, 400.0);
            assert_eq!(r["BTN"].min.x.to_string(), to, "on screen as written");
        }
        let r = run(&mut app, 700.0, 400.0);
        assert_eq!(r["BTN"].min.x, 620.0, "the write composes with Top,Right");

        let before = run(&mut app, 700.0, 400.0);
        app.root.apply_interpreter_update(StateUpdate::new("PNL", "Y", "110"), false);
        let after = run(&mut app, 700.0, 400.0);
        assert_eq!(after["PNL"].min.y, 110.0);
        assert_eq!(after["LBL"].min.y, before["LBL"].min.y - 40.0, "the container carries its contents");
        println!("056 T7.2: BTN 500 → 510 → 520 on screen; at 700 wide → 620; PNL::Y 150 → 110 carries LBL {} → {}", before["LBL"].min.y, after["LBL"].min.y);
    }

    /// Spec 056 T7.3 (AC18, AC31; R37, R39, R46, R47) — what the program is
    /// told. A 400 × 300 form with BTN at (300, 20) anchored `Top,Right` and
    /// LBL at (10, 10) anchored `Top,Left`, opened at 400 wide (Compact):
    /// nothing is mirrored and no event raised, but `me::Breakpoint` reads
    /// `Compact` and `me::FontScale` 1. Dragged to 700 (Medium): exactly one
    /// control value is mirrored — BTN's X, 600 — `Breakpoint` becomes
    /// `Medium`, and the events are `onResizing`, `onBreakpointChanged`, then
    /// `onResize` once it settles.
    #[test]
    fn the_program_is_told_the_layout_before_on_resize_056() {
        let mut f = cobolt_forms::Form::new("TELL-FORM", "Tell", 400, 300);
        f.responsive = true;
        let mut b = cobolt_forms::Control::new("BTN", cobolt_forms::ControlType::Button, 0, 0);
        b.rect = cobolt_forms::model::Rect::new(300, 20, 80, 30);
        b.set_prop("Anchor", cobolt_forms::PropValue::String("Top,Right".into()));
        let mut l = cobolt_forms::Control::new("LBL", cobolt_forms::ControlType::Label, 0, 0);
        l.rect = cobolt_forms::model::Rect::new(10, 10, 80, 20);
        f.controls.extend([b, l]);
        let (mut app, _f, pipes) = corpus_host(f, Surface::Window, None, None);
        let ctx = egui::Context::default();
        let mut step = |app: &mut FormHost, w: f32| {
            let mut input = corpus_input(egui::vec2(w, 300.0));
            input.viewports.insert(
                egui::ViewportId::ROOT,
                egui::ViewportInfo {
                    focused: Some(true),
                    inner_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(w, 300.0))),
                    ..Default::default()
                },
            );
            frame(app, &ctx, input);
        };
        let told = |pipes: &Pipes| -> Vec<(String, String, String)> {
            pipes._input_rx.try_iter().map(|u| (u.ctrl_id, u.prop, u.value)).collect()
        };
        let events = |pipes: &Pipes| -> Vec<String> {
            drain_events(pipes)
                .into_iter()
                .map(|(_, e)| e)
                .filter(|e| e.starts_with("onResiz") || e == "onBreakpointChanged")
                .collect()
        };
        for _ in 0..CORPUS_FRAMES {
            step(&mut app, 400.0);
        }
        let opened = told(&pipes);
        assert!(!opened.iter().any(|(c, _, _)| c == "BTN" || c == "LBL"), "nothing to mirror at the designed size: {opened:?}");
        assert!(opened.contains(&("TELL-FORM".into(), "Breakpoint".into(), "Compact".into())));
        assert!(opened.contains(&("TELL-FORM".into(), "FontScale".into(), "1".into())));
        assert!(events(&pipes).is_empty(), "no event for the breakpoint the form opens in");

        step(&mut app, 700.0);
        step(&mut app, 700.0);
        let resized: Vec<(String, String, String)> =
            told(&pipes).into_iter().filter(|(c, p, _)| c != "TELL-FORM" || p == "Breakpoint").collect();
        assert_eq!(
            resized,
            vec![("BTN".into(), "X".into(), "600".into()), ("TELL-FORM".into(), "Breakpoint".into(), "Medium".into())],
            "only what changed"
        );
        assert_eq!(events(&pipes), ["onResizing", "onBreakpointChanged", "onResize"]);
        println!("056 T7.3: opened — Breakpoint Compact, FontScale 1, no rect mirrored; at 700 — BTN X 600 mirrored, Breakpoint Medium; events onResizing → onBreakpointChanged → onResize");
    }

    /// Spec 056 T7.4 (AC41, R84) — the form's responsive properties written
    /// by the program take effect in the frame they arrive. An 800 × 600
    /// form in a 900 × 600 window (Medium) with A at (10, 10) and B at
    /// (200, 10), 100 × 30, and B anchored `Top,Right`:
    ///   `Breakpoint = "Compact"` pins Compact (and says so); SPACES unpins.
    ///   `FontScale = 1.5` pins the factor; 0 unpins.
    ///   `Breakpoints = "Small:0:1;Big:500:1"` replaces the table → Big.
    ///   `LayoutMode = Flex`, `FlexDirection = Column` → B directly under A.
    ///   `Responsive = 0` → the designed placement: B back at x 200.
    #[test]
    fn the_programs_form_writes_steer_the_layout_056() {
        let mut f = cobolt_forms::Form::new("PIN-FORM", "Pins", 800, 600);
        f.responsive = true;
        for (id, x) in [("A", 10), ("B", 200)] {
            let mut c = cobolt_forms::Control::new(id, cobolt_forms::ControlType::Button, 0, 0);
            c.rect = cobolt_forms::model::Rect::new(x, 10, 100, 30);
            f.controls.push(c);
        }
        f.controls[1].set_prop("Anchor", cobolt_forms::PropValue::String("Top,Right".into()));
        let (mut app, _f, pipes) = corpus_host(f, Surface::Window, None, None);
        let ctx = egui::Context::default();
        let write = |app: &mut FormHost, prop: &str, value: &str| {
            pipes._state_tx.send(StateUpdate::new("PIN-FORM", prop, value)).unwrap();
            frame(app, &ctx, corpus_input(egui::vec2(900.0, 600.0)));
            app.root.last_layout.clone().unwrap()
        };
        let l = write(&mut app, "Title", "warm-up");
        assert_eq!(l.breakpoint, "Medium");
        assert_eq!(write(&mut app, "Breakpoint", "Compact").breakpoint, "Compact");
        assert_eq!(write(&mut app, "Breakpoint", "   ").breakpoint, "Medium");
        assert_eq!(write(&mut app, "FontScale", "1.5").font_factor, 1.5);
        assert_eq!(write(&mut app, "FontScale", "0").font_factor, 1.0);
        assert_eq!(write(&mut app, "Breakpoints", "Small:0:1;Big:500:1").breakpoint, "Big");
        write(&mut app, "LayoutMode", "Flex");
        let l = write(&mut app, "FlexDirection", "Column");
        let (a, b) = (l.rects["A"], l.rects["B"]);
        assert_eq!((b.x, b.y), (a.x, a.y + a.h), "a flex column: B under A");
        pipes._state_tx.send(StateUpdate::new("PIN-FORM", "Responsive", "0")).unwrap();
        frame(&mut app, &ctx, corpus_input(egui::vec2(900.0, 600.0)));
        assert!(app.root.last_layout.is_none(), "no layout once switched off");
        assert_eq!(app.last_control_rects()["B"].min.x, 200.0, "switched off: the design");
        let changes: Vec<String> = drain_events(&pipes)
            .into_iter()
            .map(|(_, e)| e)
            .filter(|e| e == "onBreakpointChanged")
            .collect();
        assert_eq!(changes.len(), 3, "pin, unpin, new table: three changes of breakpoint");
        println!("056 T7.4: Breakpoint pin/unpin, FontScale 1.5/auto, Breakpoints replaced, LayoutMode Flex Column, Responsive off — each in its frame; 3 × onBreakpointChanged");
    }

    /// Spec 056 T7.5 (R48) — a hand-written `onResize` still composes with the
    /// layout, run by a real interpreter. The handler is PowerChat's pattern:
    /// read the form's new `Height`, set a control's `Height` from it. A 400 ×
    /// 300 form: VWR at (10, 10, 380, 200), `Top,Left`, sized by the handler to
    /// 200 + (Height − 300); BTN at (300, 250), `Bottom,Right`, placed by the
    /// layout alone. Dragged to 600 × 400: VWR 300 high, BTN at (500, 350).
    /// Dragged on to 700 × 500: VWR 400 — the handler's write, not undone by
    /// the layout, and not stuck at the last one — BTN at (600, 450).
    #[test]
    fn a_hand_written_on_resize_composes_with_the_layout_056() {
        let src = "IDENTIFICATION DIVISION.\nPROGRAM-ID. HAND-FORM.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n\
            01 COBOL-QUIT PIC 9 VALUE 0.\n01 COBOL-EVENT-ID PIC X(64) VALUE SPACES.\n\
            01 COBOL-CONTROL-ID PIC X(64) VALUE SPACES.\n01 FORM-NAME PIC X(64) VALUE 'HAND-FORM'.\n\
            01 WS-DELTA PIC S9(5) COMP-5 VALUE 0.\n01 WS-H PIC S9(5) COMP-5 VALUE 0.\n\
            PROCEDURE DIVISION.\nMAIN-PARA.\n    COBOL::\"INIT-FORM\" ( FORM-NAME )\n\
                PERFORM UNTIL COBOL-QUIT = 1\n        COBOL::\"WAIT-EVENT\" ( COBOL-EVENT-ID COBOL-CONTROL-ID )\n\
                    IF COBOL-EVENT-ID = \"onResize\"\n            COMPUTE WS-DELTA = HAND-FORM::Height - 300\n\
                        COMPUTE WS-H = 200 + WS-DELTA\n            SET VWR::Height TO WS-H\n        END-IF\n\
                    IF COBOL-EVENT-ID = \"onClose\"\n            MOVE 1 TO COBOL-QUIT\n        END-IF\n\
                END-PERFORM\n    STOP RUN.\n";
        let program = cobolt_parser::parse(cobolt_lexer::tokenize(src, cobolt_lexer::SourceFormat::Free))
            .program
            .expect("the handler parses");
        let mut f = cobolt_forms::Form::new("HAND-FORM", "Hand", 400, 300);
        f.responsive = true;
        let mut v = cobolt_forms::Control::new("VWR", cobolt_forms::ControlType::Panel, 0, 0);
        v.rect = cobolt_forms::model::Rect::new(10, 10, 380, 200);
        let mut b = cobolt_forms::Control::new("BTN", cobolt_forms::ControlType::Button, 0, 0);
        b.rect = cobolt_forms::model::Rect::new(300, 250, 80, 30);
        b.set_prop("Anchor", cobolt_forms::PropValue::String("Bottom,Right".into()));
        f.controls.extend([v, b]);
        let seed = crate::seeding::build_object_seed(&f, &f.controls, None, None);
        let (mut app, _f, pipes) = corpus_host(f, Surface::Window, None, None);
        let Pipes { ev_rx, _input_rx, _state_tx, _display_tx, .. } = pipes;
        let (display_tx, _display_rx) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            let mut interp = cobolt_runtime::interpreter::Interpreter::new_with_channels(program, ev_rx, _state_tx, display_tx);
            interp.set_input_channel(_input_rx);
            interp.seed_objects(seed);
            let _ = interp.run();
        });
        let ctx = egui::Context::default();
        let mut step = |app: &mut FormHost, w: f32, h: f32| {
            let mut input = corpus_input(egui::vec2(w, h));
            input.viewports.insert(
                egui::ViewportId::ROOT,
                egui::ViewportInfo {
                    focused: Some(true),
                    inner_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(w, h))),
                    ..Default::default()
                },
            );
            frame(app, &ctx, input);
        };
        // Run frames at `w × h` until VWR is `want` high (the handler runs on
        // its own thread), at most two seconds.
        let mut settle = |app: &mut FormHost, w: f32, h: f32, want: f32| {
            let until = Instant::now() + std::time::Duration::from_secs(2);
            loop {
                step(app, w, h);
                let r = app.last_control_rects().clone();
                if (r["VWR"].height() - want).abs() < 0.5 || Instant::now() > until {
                    return r;
                }
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
        };
        settle(&mut app, 400.0, 300.0, 200.0);
        let r = settle(&mut app, 600.0, 400.0, 300.0);
        assert_eq!((r["VWR"].height(), r["BTN"].min.x, r["BTN"].min.y), (300.0, 500.0, 350.0));
        let r = settle(&mut app, 700.0, 500.0, 400.0);
        assert_eq!((r["VWR"].height(), r["BTN"].min.x, r["BTN"].min.y), (400.0, 600.0, 450.0));
        app.root.send_event(FormEvent::new("HAND-FORM", "onClose"));
        let _ = worker.join();
        println!("056 T7.5: a real onResize handler sets VWR 200 → 300 → 400 high while the layout moves BTN to (500, 350) and (600, 450)");
    }

    /// Spec 056 T7.1 (R64) — a program's write beats a breakpoint: PNL is
    /// hidden at Compact; at 480 wide it is gone, and `SET PNL::Visible TO
    /// TRUE` brings it back although Compact is still active.
    #[test]
    fn a_program_write_beats_a_breakpoint_override_056() {
        let mut f = cobolt_forms::Form::new("BP-FORM", "Breakpoints", 800, 600);
        f.responsive = true;
        let mut p = cobolt_forms::Control::new("PNL", cobolt_forms::ControlType::Panel, 0, 0);
        p.rect = cobolt_forms::model::Rect::new(20, 20, 200, 100);
        f.controls.push(p);
        f.breakpoints[0].overrides.push(cobolt_forms::layout::breakpoints::Override {
            control: "PNL".into(),
            property: "Visible".into(),
            value: cobolt_forms::PropValue::Bool(false),
        });
        let (mut app, _f, _p) = corpus_host(f, Surface::Window, None, None);
        let ctx = egui::Context::default();
        for _ in 0..CORPUS_FRAMES {
            frame(&mut app, &ctx, corpus_input(egui::vec2(480.0, 400.0)));
        }
        assert!(!app.last_control_rects().contains_key("PNL"), "Compact hides it");
        app.root.apply_interpreter_update(StateUpdate::new("PNL", "Visible", "1"), false);
        for _ in 0..CORPUS_FRAMES {
            frame(&mut app, &ctx, corpus_input(egui::vec2(480.0, 400.0)));
        }
        assert!(app.last_control_rects().contains_key("PNL"), "the program's write wins");
    }

    /// Spec 056 T6.2 (AC32 end to end) — type scaling on a run-form window,
    /// observed through an `AutoSize` label (measured at the size it paints
    /// at). A 400 × 300 form, the label "Scaled text" at 14 pt:
    ///   `Fluid` at 600 wide (1.5×) → 21 pt → the label is ~1.5× as wide;
    ///   `Stepped` with Expanded's factor 1.25 → wider at 1100 (Expanded),
    ///   unchanged at 700 (Medium, 1.0).
    /// The designed rectangle never changes and the `FontSize` a COBOL read
    /// sees stays the designed 14 (R71, R72).
    #[test]
    fn type_scaling_reaches_the_window_and_leaves_the_design_alone_056() {
        fn fixture(scaling: &str) -> cobolt_forms::Form {
            let mut f = cobolt_forms::Form::new("TYPE-FORM", "Type", 400, 300);
            f.responsive = true;
            f.layout.insert("FontScaling".into(), cobolt_forms::PropValue::String(scaling.into()));
            f.breakpoints[2].font_factor = 1.25;
            let mut l = cobolt_forms::Control::new("LBL", cobolt_forms::ControlType::Label, 10, 10);
            l.rect = cobolt_forms::model::Rect::new(10, 10, 20, 20);
            l.set_prop("AutoSize", cobolt_forms::PropValue::Bool(true));
            l.set_prop("Caption", cobolt_forms::PropValue::String("Scaled text".into()));
            l.set_prop("FontSize", cobolt_forms::PropValue::Int(14));
            f.controls.push(l);
            f
        }
        let width = |scaling: &str, w: f32, h: f32| {
            let (mut app, _f, _p) = corpus_host(fixture(scaling), Surface::Window, None, None);
            let ctx = egui::Context::default();
            ctx.set_fonts(cobolt_forms::fonts::base_font_definitions());
            for _ in 0..CORPUS_FRAMES {
                frame(&mut app, &ctx, corpus_input(egui::vec2(w, h)));
            }
            let designed = app.root.controls.iter().find(|c| c.id == "LBL").unwrap().rect;
            assert_eq!(designed, cobolt_forms::model::Rect::new(10, 10, 20, 20), "the design is untouched");
            assert_eq!(app.root.state["LBL"].props.get("FontSize").map(String::as_str), Some("14"), "COBOL reads the designed size");
            app.last_control_rects()["LBL"].width()
        };
        let base = width("None", 400.0, 300.0);
        // AC34 (R68) — a system text factor of 1.25, injected into this one
        // host, multiplies the effective size even with `FontScaling = None`.
        let system = {
            let (mut app, _f, _p) = corpus_host(fixture("None"), Surface::Window, None, None);
            app.root.responsive.as_mut().unwrap().system_text_factor = 1.25;
            let ctx = egui::Context::default();
            ctx.set_fonts(cobolt_forms::fonts::base_font_definitions());
            for _ in 0..CORPUS_FRAMES {
                frame(&mut app, &ctx, corpus_input(egui::vec2(400.0, 300.0)));
            }
            app.last_control_rects()["LBL"].width()
        };
        assert!((system / base - 1.25).abs() < 0.15, "system factor 1.25: {base} → {system}");
        let fluid = width("Fluid", 600.0, 450.0);
        assert!((fluid / base - 1.5).abs() < 0.15, "Fluid 1.5×: {base} → {fluid}");
        let medium = width("Stepped", 700.0, 300.0);
        let expanded = width("Stepped", 1100.0, 300.0);
        assert_eq!(medium, base, "Medium's factor is 1.0");
        assert!((expanded / base - 1.25).abs() < 0.15, "Expanded's factor 1.25: {base} → {expanded}");
        println!("056 T6.2/T6.3: label {base:.0} px at 14 pt; system factor 1.25 → {system:.0}; Fluid 1.5× → {fluid:.0}; Stepped Medium → {medium:.0}, Expanded 1.25× → {expanded:.0}");
    }

    /// Spec 081 R4, R14 (AC2, AC9) — a form that is NOT responsive but carries
    /// an obsolete scaling style is laid out: designed 400 × 300 with BTN at
    /// (100, 50, 80, 30), at 800 × 450 style 3 puts it at (200, 75, 160, 45).
    /// The program writes 0 and the form is laid out no more; it writes 9 and
    /// the write is refused — the style stays 0 and the program is told so.
    #[test]
    fn an_obsolete_scaling_style_lays_out_a_form_that_is_not_responsive_081() {
        let mut f = cobolt_forms::Form::new("SCALE-FORM", "Scale", 400, 300);
        f.layout.insert("ObsoleteScalingStyle".into(), cobolt_forms::PropValue::Int(3));
        let mut b = cobolt_forms::Control::new("BTN", cobolt_forms::ControlType::Button, 100, 50);
        b.rect = cobolt_forms::model::Rect::new(100, 50, 80, 30);
        f.controls.push(b);
        assert!(!f.responsive && f.lays_out());
        let (mut app, _f, pipes) = corpus_host(f, Surface::Window, None, None);
        let ctx = egui::Context::default();
        let run = |app: &mut FormHost| {
            for _ in 0..CORPUS_FRAMES {
                frame(app, &ctx, corpus_input(egui::vec2(800.0, 450.0)));
            }
            app.last_control_rects()["BTN"]
        };
        let r = run(&mut app);
        assert_eq!((r.min.x, r.min.y, r.width(), r.height()), (200.0, 75.0, 160.0, 45.0));

        let form = app.root.form_object.clone();
        let write = |app: &mut FormHost, v: &str| {
            app.root.apply_form_window_update(&ctx, &StateUpdate::new(form.clone(), "ObsoleteScalingStyle", v), None)
        };
        assert!(write(&mut app, "0"));
        let r = run(&mut app);
        assert_eq!((r.min.x, r.min.y, r.width(), r.height()), (100.0, 50.0, 80.0, 30.0), "0: as designed");

        assert!(write(&mut app, "9"));
        let echo = pipes._input_rx.try_iter().filter(|u| u.prop == "ObsoleteScalingStyle").last().expect("told");
        assert_eq!(echo.value, "0", "9 is refused; the style stays 0");
        assert!(app.root.responsive.is_none(), "still not laid out");

        assert!(write(&mut app, "3"));
        let r = run(&mut app);
        assert_eq!((r.min.x, r.min.y), (200.0, 75.0), "3 again: scaled");
        println!("081: non-responsive form, style 3 at 800×450 → BTN (200,75,160,45); 0 → as designed; 9 refused (kept 0); 3 → scaled again");
    }

    /// Spec 056 T4.7 (AC10 host half) — one responsive fixture drawn by the
    /// run-form window and by a ContentPane whose pane is the same size gives
    /// the same rects, relative to the surface. (The rows' font column is the
    /// designed size; effective sizes are compared across the engine surfaces
    /// in `cobolt-forms/tests/responsive_precedence_056.rs`.)
    #[test]
    fn a_responsive_form_lays_out_the_same_in_a_window_and_in_a_pane_056() {
        fn fixture() -> cobolt_forms::Form {
            use cobolt_forms::{Control, ControlType, PropValue};
            let mut f = cobolt_forms::Form::new("PAR-FORM", "Parity", 600, 400);
            f.responsive = true;
            f.layout.insert("FontScaling".into(), PropValue::String("Fluid".into()));
            let mut add = |id: &str, ct: ControlType, r: (i32, i32, i32, i32), props: &[(&str, &str)]| {
                let mut c = Control::new(id, ct, r.0, r.1);
                c.rect = cobolt_forms::model::Rect::new(r.0, r.1, r.2, r.3);
                for (k, v) in props {
                    c.set_prop(*k, PropValue::String((*v).into()));
                }
                f.controls.push(c);
            };
            add("BTN", ControlType::Button, (500, 20, 80, 30), &[("Anchor", "Top,Right")]);
            add("DOCK", ControlType::Panel, (0, 360, 600, 40), &[("Dock", "Bottom")]);
            add("WIDE", ControlType::TextBox, (20, 80, 560, 24), &[("Anchor", "Top,Left,Right")]);
            add("LBL", ControlType::Label, (20, 20, 120, 24), &[("Caption", "Fluid"), ("FontSize", "15")]);
            f
        }
        let surface = egui::vec2(800.0, 500.0);
        let (mut app, _f, _p) = corpus_host(fixture(), Surface::Window, None, None);
        let ctx = egui::Context::default();
        for _ in 0..CORPUS_FRAMES {
            frame(&mut app, &ctx, corpus_input(surface));
        }
        let window = corpus_rows(&app.root, app.last_control_rects());

        // The shell grown until its pane is exactly `surface`.
        let (_, pane) = corpus_occupant(&fixture(), &None, surface);
        let grown = surface + (surface - pane.expect("an occupant owns the pane").size());
        let occupant = fixture();
        let source: FormSource = Box::new(move |_id: &str| Ok((occupant.clone(), corpus_program())));
        let shell_form = cobolt_forms::Form::new("SHELL", "Shell", grown.x as u32, grown.y as u32);
        let (mut app, _f, _p) = corpus_host(shell_form, Surface::Pane, None, Some(source));
        app.ensure_occupant("PAR-FORM").expect("builds");
        app.show_occupant(Some("PAR-FORM"));
        let mut shell = crate::shell::Shell::default();
        let ctx = egui::Context::default();
        for _ in 0..CORPUS_FRAMES {
            let mut full = ctx.run_ui(corpus_input(grown), |ui| {
                shell.show_with_host(ui, |_ui| {}, &mut app);
            });
            full.textures_delta.clear();
        }
        let pane = app.last_occupant_rect().expect("the pane");
        assert_eq!(pane.size(), surface, "the pane is the surface size");
        // Relative to the pane's origin.
        let shifted: HashMap<String, egui::Rect> = app
            .last_control_rects()
            .iter()
            .map(|(id, r)| (id.clone(), r.translate(-pane.min.to_vec2())))
            .collect();
        let occ = corpus_rows(&app.pane.occupants["PAR-FORM"].body, &shifted);
        assert_eq!(occ, window, "the pane lays the form out as the window does");
        assert!(window.iter().any(|l| l.starts_with("BTN rect=[700 20 780 50]")), "{window:?}");
        println!("056 T4.7 AC10 host: window and pane at 800×500 agree on {} rows: {}", window.len(), window.join(" · "));
    }

    /// Spec 056 T4.7 / R43 — `stretch_window_bars` spans a MenuBar and a
    /// StatusBar across the window for a form that is not responsive, and
    /// does not run on a responsive one, whose bars follow their anchors: a
    /// bar anchored `Top,Left` keeps its designed width.
    #[test]
    fn window_bars_stretch_only_when_the_form_is_not_responsive_056() {
        fn fixture(responsive: bool) -> cobolt_forms::Form {
            let mut f = cobolt_forms::Form::new("BARS-FORM", "Bars", 400, 300);
            f.responsive = responsive;
            let mut mb = cobolt_forms::Control::new("MB", cobolt_forms::ControlType::MenuBar, 0, 0);
            mb.rect = cobolt_forms::model::Rect::new(0, 0, 200, 30);
            // The MenuBar's own opt-in to spanning the window.
            mb.set_prop("MenuBarStyle", cobolt_forms::PropValue::String("Responsive".into()));
            mb.set_prop("Anchor", cobolt_forms::PropValue::String("Top,Left".into()));
            let mut sb = cobolt_forms::Control::new("SB", cobolt_forms::ControlType::StatusBar, 0, 0);
            sb.rect = cobolt_forms::model::Rect::new(0, 270, 200, 30);
            sb.set_prop("Anchor", cobolt_forms::PropValue::String("Top,Left".into()));
            f.controls.push(mb);
            f.controls.push(sb);
            f
        }
        let size = egui::vec2(600.0, 400.0);
        let widths = |responsive: bool| {
            let (mut app, _f, _p) = corpus_host(fixture(responsive), Surface::Window, None, None);
            let ctx = egui::Context::default();
            for _ in 0..CORPUS_FRAMES {
                frame(&mut app, &ctx, corpus_input(size));
            }
            let r = app.last_control_rects();
            (r["MB"].width(), r["SB"].width())
        };
        let plain = widths(false);
        let responsive = widths(true);
        assert!(plain.0 >= 600.0 && plain.1 >= 600.0, "not responsive: both bars span the window, got {plain:?}");
        assert_eq!(responsive, (200.0, 200.0), "responsive: the bars follow their anchors, nothing stretches them");
        println!("056 T4.7 R43: bars at 600 px — not responsive {plain:?}, responsive {responsive:?}");
    }

    /// Spec 056 T4.5 / R18 — the root window's minimum inner size follows the
    /// form's minimum: the builder sets it, and the host sends it again only
    /// when it changes (here a layout property written after start-up), once.
    /// A form that is not responsive never gets one.
    #[test]
    fn the_window_minimum_is_sent_again_only_when_it_changes_056() {
        let min_cmds = |cmds: Vec<egui::ViewportCommand>| -> Vec<egui::Vec2> {
            cmds.into_iter()
                .filter_map(|c| match c {
                    egui::ViewportCommand::MinInnerSize(v) => Some(v),
                    _ => None,
                })
                .collect()
        };
        let size = egui::vec2(400.0, 300.0);
        let mut f = cobolt_forms::Form::new("MIN-FORM", "Minimum", 400, 300);
        f.responsive = true;
        let (mut app, _f, _p) = corpus_host(f.clone(), Surface::Window, None, None);
        let ctx = egui::Context::default();
        let first: Vec<_> = (0..3).flat_map(|_| min_cmds(frame(&mut app, &ctx, corpus_input(size)))).collect();
        assert!(first.is_empty(), "the builder already carries the minimum: {first:?}");

        app.root
            .responsive
            .as_mut()
            .unwrap()
            .layout
            .insert("MinFormWidth".into(), cobolt_forms::PropValue::Int(360));
        let changed = min_cmds(frame(&mut app, &ctx, corpus_input(size)));
        assert_eq!(changed, vec![egui::vec2(360.0, 64.0)], "sent once when it changes");
        let after: Vec<_> = (0..3).flat_map(|_| min_cmds(frame(&mut app, &ctx, corpus_input(size)))).collect();
        assert!(after.is_empty(), "and not again: {after:?}");

        f.responsive = false;
        let (mut app, _f, _p) = corpus_host(f, Surface::Window, None, None);
        let ctx = egui::Context::default();
        let plain: Vec<_> = (0..3).flat_map(|_| min_cmds(frame(&mut app, &ctx, corpus_input(size)))).collect();
        assert!(plain.is_empty(), "a form that is not responsive keeps no minimum: {plain:?}");
        println!("056 T4.5: builder minimum 64×64, re-sent once as {:?} after MinFormWidth = 360, then silent", changed[0]);
    }

    /// Spec 056 T4.2/T4.3 — a responsive form is laid out for the surface it
    /// is drawn on: its own window, and a ContentPane that holds it.
    /// A 400×300 form with a Button anchored `Top,Right` (20 px from the right
    /// edge) and a StatusBar on its responsive default `Bottom,Left,Right`.
    #[test]
    fn a_responsive_form_lays_out_for_its_window_and_its_pane_056() {
        fn fixture() -> cobolt_forms::Form {
            let mut f = cobolt_forms::Form::new("RESP-FORM", "Responsive", 400, 300);
            f.responsive = true;
            let mut b = cobolt_forms::Control::new("BTN", cobolt_forms::ControlType::Button, 0, 0);
            b.rect = cobolt_forms::model::Rect::new(300, 20, 80, 30);
            b.set_prop("Anchor", cobolt_forms::PropValue::String("Top,Right".into()));
            let mut sb = cobolt_forms::Control::new("SB", cobolt_forms::ControlType::StatusBar, 0, 0);
            sb.rect = cobolt_forms::model::Rect::new(0, 270, 400, 30);
            f.controls.push(b);
            f.controls.push(sb);
            f
        }
        let frames = |app: &mut FormHost, ctx: &egui::Context, size: egui::Vec2| {
            for _ in 0..3 {
                frame(app, ctx, corpus_input(size));
            }
        };
        // Root window, larger than designed.
        let (mut app, _f, _p) = corpus_host(fixture(), Surface::Window, None, None);
        let ctx = egui::Context::default();
        frames(&mut app, &ctx, egui::vec2(600.0, 400.0));
        let r = app.last_control_rects().clone();
        assert_eq!(r["BTN"].max.x, 580.0, "Top,Right keeps 20 px from the window's right edge: {:?}", r["BTN"]);
        assert_eq!((r["SB"].min.y, r["SB"].width()), (370.0, 600.0), "the StatusBar spans the bottom: {:?}", r["SB"]);
        // At its designed size nothing moves (R5).
        let (mut app, _f, _p) = corpus_host(fixture(), Surface::Window, None, None);
        let ctx = egui::Context::default();
        frames(&mut app, &ctx, egui::vec2(400.0, 300.0));
        let d = app.last_control_rects().clone();
        assert_eq!((d["BTN"].min.x, d["BTN"].min.y), (300.0, 20.0));
        assert_eq!((d["SB"].min.y, d["SB"].width()), (270.0, 400.0));

        // ContentPane occupant of a plain shell.
        let occupant = fixture();
        let source: FormSource = Box::new(move |_id: &str| Ok((occupant.clone(), corpus_program())));
        let shell_form = cobolt_forms::Form::new("SHELL", "Shell", 900, 700);
        let (mut app, _f, _p) = corpus_host(shell_form, Surface::Pane, None, Some(source));
        app.ensure_occupant("RESP-FORM").expect("builds");
        app.show_occupant(Some("RESP-FORM"));
        let mut shell = crate::shell::Shell::default();
        let ctx = egui::Context::default();
        for _ in 0..3 {
            let mut full = ctx.run_ui(corpus_input(egui::vec2(900.0, 700.0)), |ui| {
                shell.show_with_host(ui, |_ui| {}, &mut app);
            });
            full.textures_delta.clear();
        }
        let pane = app.last_occupant_rect().expect("an occupant owns the pane");
        let o = app.last_control_rects().clone();
        assert!((o["BTN"].max.x - (pane.max.x - 20.0)).abs() < 1.0, "Top,Right keeps 20 px from the PANE's edge: {:?} in {pane:?}", o["BTN"]);
        assert!((o["SB"].width() - pane.width()).abs() < 1.0, "the StatusBar spans the pane: {:?} in {pane:?}", o["SB"]);
        println!(
            "056 host: window 600×400 → BTN right {:.0}, StatusBar y {:.0} w {:.0}; designed size unchanged; pane {:.0}×{:.0} → BTN right {:.0}",
            r["BTN"].max.x, r["SB"].min.y, r["SB"].width(), pane.width(), pane.height(), o["BTN"].max.x
        );
    }

    /// R18 in a ContentPane: a pane cannot refuse to grow the way a window's
    /// grip stops, so the form is laid out no larger than its limits and the
    /// rest of the pane stays empty. Before 1.80.82 the occupant took the whole
    /// pane and a stretching field ran into the chip beside it (operator,
    /// 2026-10-02, PowerDemo3's collision demo).
    #[test]
    fn a_form_in_a_pane_stops_before_its_controls_touch_056() {
        let mut f = cobolt_forms::Form::new("COLLIDE-FORM", "Collide", 400, 300);
        f.responsive = true;
        let mut add = |id: &str, ty, r: (i32, i32, i32, i32), anchor: &str| {
            let mut c = cobolt_forms::Control::new(id, ty, 0, 0);
            c.rect = cobolt_forms::model::Rect::new(r.0, r.1, r.2, r.3);
            c.set_prop("Anchor", cobolt_forms::PropValue::String(anchor.into()));
            f.controls.push(c);
        };
        // The field stretches right toward the chip: they meet 30 px wider.
        add("FIELD", cobolt_forms::ControlType::TextBox, (20, 20, 250, 30), "Top,Left,Right");
        add("CHIP", cobolt_forms::ControlType::Button, (300, 20, 80, 30), "Top,Left");
        // The notes grow down toward the line: they meet 40 px taller.
        add("NOTES", cobolt_forms::ControlType::TextBox, (20, 70, 200, 90), "Top,Bottom,Left");
        add("LINE", cobolt_forms::ControlType::Label, (20, 200, 200, 20), "Top,Left");
        let occupant = f.clone();
        let source: FormSource = Box::new(move |_id: &str| Ok((occupant.clone(), corpus_program())));
        let shell_form = cobolt_forms::Form::new("SHELL", "Shell", 900, 700);
        let (mut app, _f, _p) = corpus_host(shell_form, Surface::Pane, None, Some(source));
        app.ensure_occupant("COLLIDE-FORM").expect("builds");
        app.show_occupant(Some("COLLIDE-FORM"));
        let mut shell = crate::shell::Shell::default();
        let ctx = egui::Context::default();
        for _ in 0..3 {
            let mut full = ctx.run_ui(corpus_input(egui::vec2(900.0, 700.0)), |ui| {
                shell.show_with_host(ui, |_ui| {}, &mut app);
            });
            full.textures_delta.clear();
        }
        let pane = app.last_occupant_rect().expect("an occupant owns the pane");
        assert!(pane.width() > 500.0 && pane.height() > 400.0, "the pane is larger than the limits: {pane:?}");
        let o = app.last_control_rects().clone();
        assert!(o["FIELD"].max.x < o["CHIP"].min.x, "the field stops before the chip: {:?} {:?}", o["FIELD"], o["CHIP"]);
        assert!(o["NOTES"].max.y < o["LINE"].min.y, "the notes stop above the line: {:?} {:?}", o["NOTES"], o["LINE"]);
        assert!(o["FIELD"].width() > 250.0, "it still grew up to the limit: {:?}", o["FIELD"]);
        println!(
            "056 R18 pane {:.0}×{:.0}: field right {:.0} < chip left {:.0}; notes bottom {:.0} < line top {:.0}",
            pane.width(), pane.height(), o["FIELD"].max.x, o["CHIP"].min.x, o["NOTES"].max.y, o["LINE"].min.y
        );
    }
}

// ── A toggle written by code after a click (operator, 2026-10-06) ─────────────
#[cfg(test)]
mod toggle_write_tests {
    use super::*;
    use std::sync::mpsc;

    fn radio(id: &str, y: i32) -> cobolt_forms::Control {
        let mut c = cobolt_forms::Control::new(id, cobolt_forms::ControlType::RadioButton, 10, y);
        c.set_prop("GroupName", cobolt_forms::PropValue::String("PAYMENT".into()));
        c.set_prop("Selected", cobolt_forms::PropValue::Bool(false));
        c
    }

    fn payment_host() -> FormHost {
        let mut form = cobolt_forms::Form::new("RADIOS", "Radios", 320, 200);
        form.controls = vec![radio("Rad-Card", 10), radio("Rad-Pix", 40), radio("Rad-Boleto", 70)];
        let mut flat = Vec::new();
        crate::flatten_controls(&form.controls, &mut flat);
        let (ev_tx, _ev_rx) = mpsc::channel();
        let (input_tx, _input_rx) = mpsc::channel();
        let (_state_tx, state_rx) = mpsc::channel();
        let (_display_tx, display_rx) = mpsc::channel();
        let (form_req_tx, form_req_rx) = mpsc::channel();
        let (closed_tx, _closed_rx) = mpsc::channel();
        let (host, _form) = FormHost::new(FormHostConfig {
            form,
            flat,
            state: HashMap::new(),
            ev_tx,
            input_tx,
            state_rx,
            display_rx,
            pending: Arc::new(AtomicUsize::new(0)),
            finished: Arc::new(AtomicBool::new(false)),
            form_req_rx,
            closed_tx,
            form_req_tx,
            form_source: None,
            child_theme: None,
            child_interpreter_setup: None,
            indexed_engine: Default::default(),
            shared_rust_bridge: None,
            fx_entrance: cobolt_forms::window_fx::FxSpec::default(),
            fx_exit: cobolt_forms::window_fx::FxSpec::default(),
            fx_restore: false,
            theme_pack: None,
            surface_theme: cobolt_forms::surface_theme::liquid_glass(),
            icon_path: None,
            title_fallback: String::new(),
            hooks: Box::new(NoHooks),
            surface: Surface::Window,
        });
        host
    }

    /// What the paint shows for `id`: the design merged with the host's state,
    /// read as a toggle — the renderer's own path.
    fn painted_on(host: &FormHost, id: &str) -> bool {
        let base = host.root.controls.iter().find(|c| c.id == id).expect("control");
        let entry = host.root.state.iter().find(|(k, _)| k.eq_ignore_ascii_case(id)).map(|(_, s)| s);
        let live = match entry {
            Some(s) => cobolt_forms::render::merge_props(base, s.props.iter()),
            None => base.clone(),
        };
        cobolt_forms::model::toggle_state_of(&live)
    }

    fn stored(host: &FormHost, id: &str, prop: &str) -> Option<String> {
        let (_, s) = host.root.state.iter().find(|(k, _)| k.eq_ignore_ascii_case(id))?;
        s.props.iter().find(|(k, _)| k.eq_ignore_ascii_case(prop)).map(|(_, v)| v.clone())
    }

    /// PowerDemo3's RadioButton page: the operator clicks Card, then "Pick
    /// PIX" runs `Rad-Pix::Select()`. The handler sees PIX selected, so the
    /// screen must show it too — it showed nothing, because the click had left
    /// `Checked = 0` beside the program's `Selected = 1`.
    #[test]
    fn a_program_select_after_a_click_paints_the_radio_on() {
        let mut host = payment_host();
        // What a click on Card leaves behind: Card on, its siblings off, under
        // every spelling the renderer's click path writes.
        for (id, v) in [("Rad-Card", "1"), ("Rad-Pix", "0"), ("Rad-Boleto", "0")] {
            for prop in ["Value", "Selected", "Checked"] {
                host.root.state_entry_mut(id).set(prop, v.to_owned());
            }
        }
        // `Rad-Pix::Select()`, as the interpreter sends it: one spelling,
        // canonicalised to `Selected`, under the registry's upper-cased id.
        host.root.apply_interpreter_update(StateUpdate::new("RAD-PIX", "Selected", "1"), false);

        assert_eq!(stored(&host, "Rad-Pix", "Selected").as_deref(), Some("1"));
        assert_eq!(stored(&host, "Rad-Pix", "Checked").as_deref(), Some("1"), "both spellings agree");
        assert!(painted_on(&host, "Rad-Pix"), "PIX paints selected");
        for other in ["Rad-Card", "Rad-Boleto"] {
            assert!(!painted_on(&host, other), "{other} is cleared");
            assert_eq!(stored(&host, other, "Selected").as_deref(), Some("0"));
            assert_eq!(stored(&host, other, "Checked").as_deref(), Some("0"));
        }
        println!("click Card, then Select() on PIX: PIX painted on, Card and Boleto off; Selected/Checked agree on all 3");
    }

    /// A legacy `Checked` write from code lands the same way.
    #[test]
    fn a_legacy_checked_write_from_code_agrees_too() {
        let mut host = payment_host();
        for prop in ["Value", "Selected", "Checked"] {
            host.root.state_entry_mut("Rad-Boleto").set(prop, "0".to_owned());
        }
        host.root.apply_interpreter_update(StateUpdate::new("Rad-Boleto", "Checked", "1"), false);
        assert_eq!(stored(&host, "Rad-Boleto", "Selected").as_deref(), Some("1"));
        assert!(painted_on(&host, "Rad-Boleto"));
    }
}
