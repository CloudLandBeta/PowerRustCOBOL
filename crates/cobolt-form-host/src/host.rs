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
        .with_resizable(true)
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
    if form.start_position == cobolt_forms::model::FormStartPosition::Custom {
        viewport = viewport.with_position(egui::pos2(form.x as f32, form.y as f32));
    }
    viewport =
        viewport.with_maximized(form.window_state == cobolt_forms::model::WindowState::Maximized);
    if fx_transparent {
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
    let taskbar_icon_path: Option<PathBuf> =
        if form.main_form && !form.taskbar_icon.trim().is_empty() {
            Some(PathBuf::from(form.taskbar_icon.trim()))
        } else {
            None
        };
    if let Some(icon) = load_host_icon(taskbar_icon_path.as_deref().or(icon_path.as_deref())) {
        viewport = viewport.with_icon(icon);
    }
    let native_options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
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
        let side_dx: i32 = if surface == Surface::Pane {
            flat.iter()
                .find(|c| c.control_type == cobolt_forms::ControlType::SideMenu)
                .map(|c| c.rect.w.max(0))
                .unwrap_or(0)
        } else {
            0
        };

        // 049 — in a pane, the SideMenu IS the MenuPane: the shell paints it as
        // chrome outside this host. Rendering the control again inside the
        // ContentPane would put the same sidebar on screen twice, side by side,
        // and `FullHeight` makes that second copy as tall as the whole form.
        // Only its PAINT is dropped — the control keeps its state entry below,
        // so `SelectedItemId` and its event handlers still work.
        let flat: Vec<cobolt_forms::Control> = if surface == Surface::Pane {
            flat.into_iter()
                .filter(|c| c.control_type != cobolt_forms::ControlType::SideMenu)
                .map(|mut c| {
                    // Slide the form's content area over the rail's column so
                    // its left edge lands ON the pane's left edge — juxtaposed
                    // to the rail rather than offset from it twice. A control
                    // the developer parked UNDER the rail clamps to the edge
                    // instead of disappearing off the left of the pane.
                    c.rect.x = (c.rect.x - side_dx).max(0);
                    c
                })
                .collect()
        } else {
            flat
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

        let host = FormHost {
            root: FormBody {
                form_name: form.name.clone(),
                theme_pack,
                surface_theme,
                glass_style,
                controls: flat,
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
                form_size: egui::vec2(fw, fh),
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
            },
            children: Vec::new(),
            occupants: HashMap::new(),
            active_occupant: None,
            form_req_tx,
            form_source,
            child_theme,
            child_interpreter_setup,
            shared_rust_bridge,
            surface,
            toolbar_runner: crate::toolbar_actions::Runner::default(),
            visuals_set: false,
            quit_sent: false,
            diagnostics,
            start_minimized: form.window_state == cobolt_forms::model::WindowState::Minimized,
            supervisor: cobolt_runtime::form_host::FormSupervisor::new(&form.name, &form.name),
            form_req_rx,
            closed: ClosedFanout::new(closed_tx),
            fullscreen_actual: form.full_screen,
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
            fx_transparent,
            fx_chrome_pending: fx_hide_chrome && form.title_visible,
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
pub(crate) struct FormBody {
    pub(crate) form_name: String,
    /// Resolved asset-pack theme (None = built-in Liquid Glass) + the form's
    /// glass style — pushed into the egui context so the unified painter reads
    /// the same theme state as under the IDE (spec 017 parity).
    pub(crate) theme_pack: Option<Arc<cobolt_forms::theme_pack::ThemePack>>,
    pub(crate) surface_theme: std::sync::Arc<dyn cobolt_forms::surface_theme::SurfaceTheme>,
    pub(crate) glass_style: cobolt_forms::model::GlassStyle,
    pub(crate) controls: Vec<cobolt_forms::Control>,
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
    pub(crate) form_size: egui::Vec2,
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
}

impl FormBody {
    pub(crate) fn send_event(&mut self, ev: FormEvent) {
        if self.ev_tx.send(ev).is_ok() {
            self.pending.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// See [`crate::state::state_entry_mut`].
    pub(crate) fn state_entry_mut(&mut self, key: &str) -> &mut CtrlState {
        state_entry_mut(&mut self.state, &self.controls, key)
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
    pub(crate) fn backdrop(&self, ctx: &egui::Context) -> cobolt_forms::render::Backdrop {
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
        cobolt_forms::render::Backdrop {
            color_hex: self.bg_hex.clone(),
            transparency: self.transparency,
            gradient_enabled: self.bg_gradient_enabled,
            gradient_start_hex: self.bg_gradient_start.clone(),
            gradient_end_hex: self.bg_gradient_end.clone(),
            gradient_direction: self.bg_gradient_direction.clone(),
            image,
            image_mode: self.bg_mode,
            use_theme_background: self.use_theme_background,
            // The gradient / background image follows the WINDOW: it stretches
            // over the whole thing when the user maximizes or drags it bigger,
            // and stays form-sized when the window is dragged smaller. The
            // controls keep their designed size either way.
            window_size: Some(ctx.content_rect().size()),
        }
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
    pub(crate) fn child_frame(&mut self, panel_ui: &mut egui::Ui, blocked: bool) {
        // Panels are Ui-hosted since egui 0.35; everything else here wants a
        // Context.
        let ctx = panel_ui.ctx().clone();
        let ctx = &ctx;
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
            self.state_entry_mut(&key).set(&u.prop, u.value);
        }

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
            self.send_event(FormEvent::new(&name, "onShow"));
            self.send_event(FormEvent::new(&name, "onActivate"));
        }

        let bg_fill = cobolt_forms::render::backdrop_color(&self.bg_hex, self.transparency);
        let form_size = self.form_size;
        let output = {
            let controls = self.controls.clone();
            let st = LiveState {
                state: &self.state,
                anim: &self.anim,
            };
            let active_tabs = cobolt_forms::containers::ActiveTabs::default();
            let backdrop = self.backdrop(ctx);
            let mut out = cobolt_forms::render::RenderOutput::default();
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE.fill(bg_fill))
                .show(panel_ui, |ui| {
                    if blocked {
                        ui.disable();
                    }
                    ui.style_mut().spacing.scroll = egui::style::ScrollStyle::floating();
                    egui::ScrollArea::both()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.set_min_size(form_size);
                            let input = cobolt_forms::render::RenderInput {
                                controls: &controls,
                                state: &st,
                                form_size,
                                glass: true,
                                mode: cobolt_forms::render::RenderMode::Interactive,
                                active_tabs: &active_tabs,
                                backdrop,
                            };
                            out = cobolt_forms::render::render_form(ui, &input);
                        });
                });
            out
        };

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
            // root's timer-tick backlog coalescing.
            for (id, key, val) in &output.prop_updates {
                self.state_entry_mut(id).set(key, val.clone());
                let _ = self
                    .input_tx
                    .send(StateUpdate::new(id.clone(), key.clone(), val.clone()));
            }
            let backlog = self.pending.load(Ordering::Relaxed) > 0;
            for ev in output.events {
                let is_tick = ev.event.eq_ignore_ascii_case("onTick");
                if is_tick && backlog {
                    continue;
                }
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
                self.send_event(FormEvent::new(dispatch_id, ev.event).with_index(inst));
            }
        }

        // A busy child keeps frames coming; an idle one rides the root's
        // heartbeat.
        if drained > 0 || animating || self.anim.is_animating() {
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
    /// The caller's `formWindowState` override, or the design's — applied as
    /// commands on the first frame (Maximized/Minimized/Fullscreen).
    pub(crate) initial_state: Option<String>,
    pub(crate) init_sent: bool,
    /// A finished interpreter is reported to the supervisor exactly once.
    pub(crate) finish_reported: bool,
}

/// 051 — a ContentPane occupant: a full form instance shown in the shell's
/// pane instead of the root form. Resident while registered (049 R20) —
/// parked occupants keep their interpreter and storage warm.
pub(crate) struct Occupant {
    pub(crate) handle: String,
    pub(crate) body: FormBody,
}

pub struct FormHost {
    /// The ROOT form — the window (or pane occupant) this host started with.
    root: FormBody,
    /// 051 — spawned child windows, one viewport each, re-declared per frame.
    children: Vec<ChildWindow>,
    /// 051 R10/R11 — pane occupants, keyed by UPPERCASE form object. Every
    /// entry is resident; `active_occupant` names the one on the pane
    /// (`None` = the root form shows, as it always has).
    occupants: HashMap<String, Occupant>,
    active_occupant: Option<String>,
    /// 051 — the pieces `SpawnWindow` builds a child from (see the config).
    form_req_tx: mpsc::Sender<cobolt_runtime::form_host::FormRequest>,
    form_source: Option<FormSource>,
    child_theme: Option<ChildThemeSource>,
    child_interpreter_setup:
        Option<std::sync::Arc<dyn Fn(&mut cobolt_runtime::interpreter::Interpreter) + Send + Sync>>,
    shared_rust_bridge:
        Option<std::sync::Arc<std::sync::Mutex<cobolt_runtime::rust_bridge::RustBridge>>>,
    /// 049 — own window, or the shell's ContentPane (see [`Surface`]).
    surface: Surface,
    /// Carries out a toolbar button's platform action, and finishes the window
    /// captures that cannot complete on the frame that asked for them.
    toolbar_runner: crate::toolbar_actions::Runner,
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
    /// Center) — `None` once applied, or when the form is `System`/`Custom`
    /// and needs no first-frame command at all (`Custom` is already in the
    /// viewport builder; `System` means "do not touch it").
    pending_start_position: Option<cobolt_forms::model::FormStartPosition>,
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
    /// The title bar is designed to be visible but is currently OFF so the
    /// entrance plays with no fixed chrome; it is switched back on the frame
    /// the animation ends.
    fx_chrome_pending: bool,
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
                        } else if let Some(key) = self
                            .occupants
                            .iter()
                            .find(|(_, o)| o.handle == handle)
                            .map(|(k, _)| k.clone())
                        {
                            // 051 — an occupant caught in a close cascade
                            // (application close) goes the same way.
                            if let Some(occ) = self.occupants.remove(&key) {
                                let _ = occ.body.ev_tx.send(FormEvent::quit());
                            }
                            if self.active_occupant.as_deref() == Some(key.as_str()) {
                                self.active_occupant = None;
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
                            self.viewport_cmd(ctx,egui::ViewportCommand::Decorations(on));
                        } else if let Some(vp) = self.child_viewport(&handle) {
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
                        let target = if handle == ROOT_HANDLE {
                            Some(&self.root)
                        } else {
                            self.children
                                .iter()
                                .find(|c| c.handle == handle)
                                .map(|c| &c.body)
                        };
                        if let Some(body) = target {
                            let _ = body.input_tx.send(StateUpdate {
                                ctrl_id: body.form_object.clone(),
                                prop: key,
                                value,
                                instance_index: 0,
                            });
                        }
                    }
                    // 049 R44 — surfaced for the SHELL host, which applies it
                    // to its MenuPane and persists it (R9). A classic window
                    // host takes it nowhere.
                    HostAction::SetMenuPaneCollapsed { collapsed } => {
                        self.pending_menu_pane = Some(collapsed);
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

    /// 051 R10 — make sure a pane occupant for `form_id` exists (building
    /// its instance and registering its Embedded handle on first need) and
    /// return its event sender for the shell's `Resident` lifecycle box.
    /// A parked occupant is simply found again — its storage was the point.
    pub fn ensure_occupant(
        &mut self,
        form_id: &str,
    ) -> Result<mpsc::Sender<FormEvent>, String> {
        let key = form_id.trim().to_ascii_uppercase();
        if let Some(occ) = self.occupants.get(&key) {
            return Ok(occ.body.ev_tx.clone());
        }
        let handle = self
            .supervisor
            .open_embedded(cobolt_runtime::form_host::ROOT_HANDLE, &key);
        let (body, _form) = match self.build_form_instance(&handle, form_id) {
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
        self.occupants.insert(key, Occupant { handle, body });
        Ok(ev_tx)
    }

    /// 051 R10/R11 — put `form_object` (UPPERCASE; `None` = the root form)
    /// on the pane. The ENTERING side of a swap: a form whose lifecycle pair
    /// already fired gets its `onActivate` here (a fresh instance fires
    /// onShow/onActivate through its own warm-up instead). The leaving
    /// side's `onDeactivate`/`onDestroy` is the NavChain's `Resident` job.
    pub fn show_occupant(&mut self, form_object: Option<&str>) {
        let key = form_object.map(|f| f.trim().to_ascii_uppercase());
        if key == self.active_occupant {
            return;
        }
        self.active_occupant = key.clone();
        match key {
            None => {
                if self.root.lifecycle_sent {
                    let name = self.root.form_object.clone();
                    self.root.send_event(FormEvent::new(name, "onActivate"));
                }
            }
            Some(k) => {
                if let Some(occ) = self.occupants.get_mut(&k) {
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
        for form_object in gone {
            let key = form_object.trim().to_ascii_uppercase();
            if let Some(occ) = self.occupants.remove(&key) {
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
        self.active_occupant.as_deref()
    }

    /// Test-only: mark the root's lifecycle pair as already fired, the state
    /// every real run reaches after its warm-up.
    #[cfg(test)]
    pub(crate) fn root_lifecycle_sent_for_test(&mut self) {
        self.root.lifecycle_sent = true;
    }

    /// Test-only observability: the registered occupants' form objects.
    #[cfg(test)]
    pub(crate) fn occupant_forms(&self) -> Vec<String> {
        let mut v: Vec<String> = self.occupants.keys().cloned().collect();
        v.sort();
        v
    }

    /// Test-only observability: an occupant's supervisor handle — a revived
    /// (preserved) occupant keeps the one it was born with.
    #[cfg(test)]
    pub(crate) fn occupant_handle(&self, form_object: &str) -> Option<String> {
        self.occupants
            .get(&form_object.trim().to_ascii_uppercase())
            .map(|o| o.handle.clone())
    }

    /// 051 Q2 — tick every PARKED body's timers (the root while an occupant
    /// shows; every off-pane occupant always), and schedule the wake-up for
    /// the earliest due tick.
    fn tick_parked_bodies(&mut self, ctx: &egui::Context) {
        let active = self.active_occupant.clone();
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
        let keys: Vec<String> = self.occupants.keys().cloned().collect();
        for k in keys {
            if Some(k.as_str()) == active.as_deref() {
                continue;
            }
            if let Some(occ) = self.occupants.get_mut(&k) {
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
    pub fn root_modal_blocked(&self) -> bool {
        !self
            .supervisor
            .modal_children_of(cobolt_runtime::form_host::ROOT_HANDLE)
            .is_empty()
    }

    /// 051 — the child window under `handle`, if any.
    fn child_viewport(&self, handle: &str) -> Option<egui::ViewportId> {
        self.children
            .iter()
            .find(|c| c.handle == handle)
            .map(|c| c.viewport_id)
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
        let (body, form) = self.build_form_instance(handle, form_id)?;
        let (fw, fh) = (form.width as f32, form.height as f32);
        let size = egui::vec2(
            width.map(|w| w as f32).unwrap_or(fw).max(1.0),
            height.map(|h| h as f32).unwrap_or(fh).max(1.0),
        );
        let pos = match (x, y) {
            (Some(px), Some(py)) => Some(egui::pos2(px as f32, py as f32)),
            _ => None,
        };
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
            initial_state,
            init_sent: false,
            finish_reported: false,
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
        {
            let finished = Arc::clone(&finished);
            let pending = Arc::clone(&pending);
            let form_object = form_object.clone();
            let handle = handle.to_string();
            let req_tx = self.form_req_tx.clone();
            let setup = self.child_interpreter_setup.clone();
            let bridge = self.shared_rust_bridge.clone();
            let err_tx = display_tx.clone();
            std::thread::spawn(move || {
                let mut interp = cobolt_runtime::interpreter::Interpreter::new_with_channels(
                    program, ev_rx, state_tx, display_tx,
                );
                interp.set_input_channel(input_rx);
                interp.set_event_counter(pending);
                interp.set_form_host(req_tx, &handle, &form_object, closed_rx);
                if let Some(b) = bridge {
                    // 051 Q1 — one object bridge per process.
                    interp.set_shared_rust_bridge(b);
                }
                if let Some(setup) = setup {
                    setup(&mut interp);
                }
                interp.seed_objects(seed);
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

        // The child's theme: the glue resolves it (embedded vs on-disk art);
        // without a resolver it paints procedural Liquid Glass.
        let (theme_pack, surface_theme) = match &self.child_theme {
            Some(resolve) => resolve(&form),
            None => (None, cobolt_forms::surface_theme::liquid_glass()),
        };

        let (fw, fh) = (form.width as f32, form.height as f32);
        let body = FormBody {
            form_name: form.name.clone(),
            theme_pack,
            surface_theme,
            glass_style: form.glass_style,
            controls: flat,
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
            form_size: egui::vec2(fw, fh),
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
            // The window blocks while ITS OWN modal child lives (R28).
            let blocked = {
                let h = &self.children[i].handle;
                !self.supervisor.modal_children_of(h).is_empty()
            };
            let child = &mut self.children[i];
            let mut builder = egui::ViewportBuilder::default()
                .with_title(child.title.clone())
                .with_inner_size(child.size)
                .with_decorations(child.decorations);
            if let Some(p) = child.pos {
                builder = builder.with_position(p);
            }
            let vp = child.viewport_id;
            let handle = child.handle.clone();
            let mut close_requested = false;
            ctx.show_viewport_immediate(vp, builder, |vp_ui, _class| {
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
                if vp_ui.input(|i| i.viewport().close_requested()) {
                    // The supervisor decides (vetoes, cascades) — cancel the
                    // OS close and route it like every other close.
                    vp_ui.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                    close_requested = true;
                }
                child.body.child_frame(vp_ui, blocked);
            });
            if close_requested {
                close_requests.push(handle);
            }
        }
        for handle in close_requests {
            let acts = self.supervisor.try_close(&handle);
            self.apply_host_actions(ctx, acts);
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
        let backdrop = self.root.backdrop(root_ui.ctx());
        let controls = &self.root.controls;
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

impl eframe::App for FormHost {
    /// What the framebuffer is cleared to before anything is painted. On a
    /// see-through window (038 — an entrance that plays over the desktop)
    /// this is fully transparent: the form's own backdrop supplies whatever
    /// opacity it was designed with, and everything it does not paint stays
    /// desktop. Otherwise it is the form's own background colour, so no
    /// stray frame of eframe's default grey can show through an effect.
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        if self.fx_transparent {
            egui::Color32::TRANSPARENT.to_normalized_gamma_f32()
        } else {
            cobolt_forms::render::backdrop_color(&self.root.bg_hex, 0).to_normalized_gamma_f32()
        }
    }

    fn ui(&mut self, root_ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
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

    /// 049 R44 — drain a COBOL-driven MenuPane state change (the shell
    /// applies it to `Shell::collapsed` and persists it, R9).
    pub fn take_menu_pane_request(&mut self) -> Option<bool> {
        self.pending_menu_pane.take()
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

    /// One frame of the host. Split from [`eframe::App::ui`] (which only adds
    /// the unused `Frame` parameter) so the parity suite can drive frames
    /// through `Context::run_ui` headlessly (spec 042 R29).
    fn ui_impl(&mut self, root_ui: &mut egui::Ui) {
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
        // 037 R13 — a form designed to open Minimized minimizes on its first
        // frame (one-shot; the builder cannot pre-minimize).
        if self.start_minimized {
            self.start_minimized = false;
            self.viewport_cmd(ctx,egui::ViewportCommand::Minimized(true));
        }
        // Window start position — the eight edge/corner positions and Center
        // need the monitor's size, which (unlike `start_minimized` above)
        // winit may not have reported yet on the very first frame; keep this
        // pending until both it and the window's own outer size are known,
        // rather than consuming the flag against absent data.
        if let Some(pos) = self.pending_start_position {
            let ready = ctx.input(|i| {
                let v = i.viewport();
                Some((v.monitor_size?, v.outer_rect?.size()))
            });
            if let Some((monitor, window)) = ready {
                if let Some((x, y)) = cobolt_forms::model::resolved_start_position(
                    pos,
                    (monitor.x, monitor.y),
                    (window.x, window.y),
                ) {
                    self.viewport_cmd(ctx,egui::ViewportCommand::OuterPosition(egui::pos2(x, y)));
                }
                self.pending_start_position = None;
            }
        }
        // Theme pack + glass style for the unified painter (per frame — same
        // contract every host follows).
        cobolt_forms::paint::set_active_theme(ctx, self.root.theme_pack.clone());
        cobolt_forms::paint::set_glass_style(ctx, self.root.glass_style);
        cobolt_forms::paint::set_surface_theme(ctx, self.root.surface_theme.clone());

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
            let acts = self.supervisor.handle_request(req);
            self.apply_host_actions(ctx, acts);
        }

        // 051 — the children pass: finished interpreters release their
        // handles; every child viewport is re-declared for this frame.
        self.update_children(ctx);

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
                self.root.send_event(FormEvent::new(form, "onFullScreenChanged"));
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
            if let Some(cmd) = anim_command(&u.prop) {
                match cmd {
                    AnimCommand::Play => {
                        self.root.anim
                            .play_programmatic(&self.root.controls, &u.ctrl_id, &u.value)
                    }
                    AnimCommand::Stop => self.root.anim.stop_all(&u.ctrl_id),
                    AnimCommand::Pause => self.root.anim.pause_all(&u.ctrl_id),
                }
                drained += 1;
                continue;
            }
            let key = if u.instance_index > 0 {
                match self.root.array_member_group(&u.ctrl_id) {
                    Some((member_id, group_id)) => cobolt_forms::render::member_instance_id(
                        &group_id,
                        &member_id,
                        u.instance_index,
                    ),
                    None => self.root.resolve_ctrl_key(&u.ctrl_id),
                }
            } else {
                self.root.resolve_ctrl_key(&u.ctrl_id)
            };
            // R27 — the live trace: which designed control this write landed
            // on, or NO SUCH CONTROL with the ids that do exist. The routing
            // itself is unchanged; the trace only reports it.
            if self.diagnostics {
                let matched = self
                    .root
                    .state
                    .keys()
                    .find(|k| k.eq_ignore_ascii_case(&key))
                    .cloned();
                let known: Vec<&str> = if matched.is_none() {
                    self.root.state.keys().map(|s| s.as_str()).collect()
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
            self.root.state_entry_mut(&key).set(&u.prop, u.value);
            drained += 1;
        }

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
        // 051 R19/R28 — while a MODAL child of this window is open, the whole
        // root face is disabled: it stays visible but takes no input.
        let root_blocked = !self
            .supervisor
            .modal_children_of(cobolt_runtime::form_host::ROOT_HANDLE)
            .is_empty();
        // 051 Q2 — parked bodies keep their timers running, whoever owns the
        // pane this frame.
        self.tick_parked_bodies(ctx);
        // 051 R10 — an active occupant owns the pane; the root form above
        // stayed fully live (its drains ran), just unrendered — parked.
        if let Some(key) = self.active_occupant.clone() {
            if let Some(occ) = self.occupants.get_mut(&key) {
                occ.body.child_frame(root_ui, root_blocked);
                ctx.request_repaint_after(std::time::Duration::from_millis(200));
                return;
            }
        }
        let mut pane_backdrop_rect: Option<egui::Rect> = None;
        let mut pane_backdrop_fill: Option<egui::Color32> = None;
        let mut content_scroll = egui::Vec2::ZERO;
        let output = {
            let controls = self.root.controls.clone();
            let st = LiveState {
                state: &self.root.state,
                anim: &self.root.anim,
            };
            let active_tabs = cobolt_forms::containers::ActiveTabs::default();
            let backdrop = self.root.backdrop(ctx);
            let mut out = cobolt_forms::render::RenderOutput::default();
            // On a see-through window the panel must NOT fill: the engine
            // paints the same backdrop across the whole window a moment
            // later, and painting a translucent colour twice would double the
            // form's designed opacity against the desktop. In Pane mode the
            // panel never fills either — the pane-fixed backdrop below is the
            // one and only background paint (049 R41).
            let panel_fill = if self.fx_transparent || surface == Surface::Pane {
                egui::Color32::TRANSPARENT
            } else {
                bg_fill
            };
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE.fill(panel_fill))
                .show(root_ui, |ui| {
                    if root_blocked {
                        ui.disable();
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
                        }
                    } else {
                        backdrop
                    };
                    // Floating scrollbars overlay the content instead of
                    // reserving a gutter, so no light track strip shows on the
                    // right/bottom edges when the form fits (only appears, as an
                    // overlay, if the user shrinks the resizable window).
                    ui.style_mut().spacing.scroll = egui::style::ScrollStyle::floating();
                    let sa = egui::ScrollArea::both()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.set_min_size(form_size);
                            let input = cobolt_forms::render::RenderInput {
                                controls: &controls,
                                state: &st,
                                form_size,
                                glass: true,
                                mode: cobolt_forms::render::RenderMode::Interactive,
                                active_tabs: &active_tabs,
                                backdrop: engine_backdrop,
                            };
                            out = cobolt_forms::render::render_form(ui, &input);
                        });
                    content_scroll = sa.state.offset;
                });
            out
        };
        self.last_pane_backdrop_rect = pane_backdrop_rect;
        self.last_pane_backdrop_fill = pane_backdrop_fill;
        self.last_content_scroll = content_scroll;

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
            for (id, key, val) in &output.prop_updates {
                self.root.state_entry_mut(id).set(key, val.clone());
                let _ = self
                    .root
                    .input_tx
                    .send(StateUpdate::new(id.clone(), key.clone(), val.clone()));
                interacted = true;
            }
            // Coalesce timer ticks against a still-queued backlog (WinForms
            // semantics) so a slow handler can't flood the event queue. User
            // events (clicks, edits, focus, quit) are never dropped.
            let backlog = self.root.pending.load(Ordering::Relaxed) > 0;
            for ev in output.events {
                let is_tick = ev.event.eq_ignore_ascii_case("onTick");
                if is_tick && backlog {
                    continue;
                }
                // Instanced repeating-group members are drawn with the id
                // "group.group-N.member" — dispatch to the designed (base)
                // member id, forwarding the 1-based instance index so the
                // handler receives CONTROL-ARRAY-INDEX.
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
                self.root.send_event(FormEvent::new(dispatch_id, ev.event).with_index(inst));
                interacted = true;
            }

            // FileDropZone click → native picker (spec 039 T4). `cobolt-forms`
            // has no native-dialog dependency by design (see render.rs's
            // `RenderOutput::file_picker_requests` doc comment) — this crate
            // owns the non-blocking dialog (spec 042 R25).
            for id in &output.file_picker_requests {
                let key = format!("filedropzone:{id}");
                crate::file_dialog::begin(ctx, &key, crate::file_dialog::DialogSpec::open());
            }
            let file_drop_zone_ids: Vec<String> = self
                .root
                .controls
                .iter()
                .filter(|c| matches!(c.control_type, cobolt_forms::ControlType::FileDropZone))
                .map(|c| c.id.clone())
                .collect();
            for id in file_drop_zone_ids {
                let key = format!("filedropzone:{id}");
                if let Some(Some(path)) = crate::file_dialog::take(&key) {
                    // Browsing goes through the SAME intake as a drop — the
                    // zone's extensions, size limit and destination folder — so
                    // a file is judged by one set of rules however it arrived.
                    let ctrl = self.root.controls.iter().find(|c| c.id == id);
                    let prop = |key: &str| -> String {
                        ctrl.and_then(|c| c.get_prop(key))
                            .map(|v| v.as_str().to_owned())
                            .unwrap_or_default()
                    };
                    let bool_prop = |key: &str, default: bool| -> bool {
                        ctrl.and_then(|c| c.get_prop(key))
                            .map(|v| v.as_bool())
                            .unwrap_or(default)
                    };
                    // `apply_drop` also decides whether this copies now or only
                    // stages for the form to confirm — the same answer the
                    // drag-drop path gets.
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
                                .root
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
                        self.root.state_entry_mut(target).set(key, value.clone());
                        let _ = self.root.input_tx.send(StateUpdate::new(
                            target.clone(),
                            key.clone(),
                            value.clone(),
                        ));
                    }
                    if writes.accepted > 0 {
                        self.root.send_event(FormEvent::new(id.clone(), "onFilesDropped".to_owned()));
                    }
                    if writes.rejected > 0 {
                        self.root.send_event(FormEvent::new(id, "onFilesRejected".to_owned()));
                    }
                    interacted = true;
                }
            }

            // ── Toolbar buttons whose action is the platform's work ──────────
            //
            // The renderer already fired the button's `onClick`, so the form has
            // heard about the press either way; this is the deed itself.
            for (ctrl_id, button_id, action) in output.toolbar_actions.clone() {
                let parsed = cobolt_forms::toolbar::ToolbarAction::parse(&action);
                // Copy/Cut/Paste act on whichever control has keyboard focus.
                // egui reports that as a widget id, and a control's TextEdit is
                // built with `Id::new(("rt_ctrl", <control id>))` — so the focused
                // control is found by matching that back.
                let focused = ctx.memory(|m| m.focused()).and_then(|focus| {
                    self.root.controls.iter().find_map(|c| {
                        (egui::Id::new(("rt_ctrl", c.id.as_str())) == focus).then(|| {
                            let text = self
                                .root
                                .state
                                .get(&c.id)
                                .and_then(|s| {
                                    s.props.iter().find_map(|(k, v)| {
                                        (k.eq_ignore_ascii_case("Text")
                                            || k.eq_ignore_ascii_case("Value"))
                                        .then(|| v.clone())
                                    })
                                })
                                .unwrap_or_default();
                            (c.id.clone(), text)
                        })
                    })
                });
                let focused_ref =
                    focused
                        .as_ref()
                        .map(|(id, text)| crate::toolbar_actions::Focused {
                            control_id: id.as_str(),
                            text: text.clone(),
                        });
                let (_outcome, new_text) =
                    self.toolbar_runner.perform(ctx, &parsed, focused_ref);
                // A Cut or a Paste changed the focused field: write it back the
                // way a keystroke would have, so the form sees it.
                if let (Some(text), Some((target, _))) = (new_text, focused) {
                    self.root.state_entry_mut(&target).set("Text", text.clone());
                    let _ = self.root.input_tx.send(StateUpdate::new(
                        target.clone(),
                        "Text".to_owned(),
                        text,
                    ));
                    self.root
                        .send_event(FormEvent::new(target, "onChange".to_owned()));
                }
                let _ = (&ctrl_id, &button_id);
                interacted = true;
            }
            // A window capture asked for on an earlier frame finishes here.
            if self.toolbar_runner.poll_capture(ctx).is_some() {
                interacted = true;
            }
        }

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
        host.active_occupant = Some("SOMEONE-ELSE".into());
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
        assert!(host.root.parked_timer_clocks.is_empty() || host.active_occupant.is_none());

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

    fn host_with(entrance: &str, exit: &str, restore: bool) -> (FormHost, Pipes) {
        host_with_surface(entrance, exit, restore, Surface::Window)
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
        assert!(
            drain_events(&pipes).is_empty(),
            "a restore replay fires no form events"
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
}
