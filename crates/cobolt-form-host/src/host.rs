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
pub struct FormHostConfig {
    /// The designed form — window properties, backdrop, fx opt-out and the
    /// controls' designed tree all come from here.
    pub form: cobolt_forms::Form,
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
    pub surface_style: cobolt_forms::paint::SurfaceStyle,
    /// Project icon path, if the glue has one (`--icon` / bundled asset).
    pub icon_path: Option<PathBuf>,
    /// Window title when the designed `form.title` is blank (spec 042 R17):
    /// run-form passes an empty string (a blank title stays blank, as ever);
    /// a compiled application passes `"{AppName} v{Version}"`.
    pub title_fallback: String,
    /// The per-host seam (R30).
    pub hooks: Box<dyn HostHooks>,
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
            fx_entrance,
            fx_exit,
            fx_restore,
            theme_pack,
            surface_style,
            icon_path: _,
            title_fallback: _,
            hooks,
        } = config;

        let glass_style = form.glass_style;
        let form_object = form.name.trim().to_ascii_uppercase();
        let (fw, fh) = (form.width as f32, form.height as f32);

        // R27 — with diagnostics on, say what this window IS before anything
        // runs.
        let diagnostics = crate::diagnostics::frame_diagnostics_enabled();
        if diagnostics {
            let ids: Vec<&str> = flat.iter().map(|c| c.id.as_str()).collect();
            crate::diagnostics::launch_preamble(&form, &ids);
        }

        let (fx_hide_chrome, fx_transparent) = fx_window_flags(&fx_entrance);

        let host = FormHost {
            form_name: form.name.clone(),
            theme_pack,
            surface_style,
            glass_style,
            visuals_set: false,
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
            quit_sent: false,
            db_dumped: false,
            diagnostics,
            anim: cobolt_forms::anim::AnimRuntime::new(fw, fh),
            anim_started: false,
            last_frame: None,
            hovered: std::collections::HashSet::new(),
            start_minimized: form.window_state == cobolt_forms::model::WindowState::Minimized,
            supervisor: cobolt_runtime::form_host::FormSupervisor::new(&form.name, &form.name),
            form_req_rx,
            closed_tx,
            form_object,
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
        };
        (host, form)
    }
}

// ── FormHost ──────────────────────────────────────────────────────────────────

pub struct FormHost {
    form_name: String,
    /// Resolved asset-pack theme (None = built-in Liquid Glass) + the form's
    /// glass style — pushed into the egui context so the unified painter reads
    /// the same theme state as under the IDE (spec 017 parity).
    theme_pack: Option<Arc<cobolt_forms::theme_pack::ThemePack>>,
    surface_style: cobolt_forms::paint::SurfaceStyle,
    glass_style: cobolt_forms::model::GlassStyle,
    visuals_set: bool,
    controls: Vec<cobolt_forms::Control>,
    state: HashMap<String, CtrlState>,
    bg_hex: String,
    bg_gradient_enabled: bool,
    bg_gradient_start: String,
    bg_gradient_end: String,
    bg_gradient_direction: String,
    transparency: u8,
    bg_image: String,
    bg_mode: cobolt_forms::model::BgImageMode,
    /// The form's `UseThemeBackground` opt-in — the pack's background art
    /// replaces the form's own image when the active theme provides one.
    use_theme_background: bool,
    form_size: egui::Vec2,
    ev_tx: mpsc::Sender<FormEvent>,
    input_tx: mpsc::Sender<StateUpdate>,
    state_rx: mpsc::Receiver<StateUpdate>,
    display_rx: mpsc::Receiver<String>,
    /// Events queued for the interpreter — lets the render Timer arm coalesce
    /// ticks against a backlog and drives the fast-repaint branch below.
    pending: Arc<AtomicUsize>,
    /// Set by the interpreter thread when the program ends (STOP RUN or error).
    finished: Arc<AtomicBool>,
    /// When the window opened. Input is ignored for a short warm-up so a click
    /// in progress as the window appears can't fire a phantom event.
    start: std::time::Instant,
    lifecycle_sent: bool,
    quit_sent: bool,
    /// One-shot guard for the `COBOLT_DATABIND_TRACE` render-side dump.
    db_dumped: bool,
    /// `COBOLT_FRAME_DIAGNOSTICS` — live per-update trace (R27). Without it a
    /// host is a black box: "the label did not change" cannot be told apart
    /// from "the handler never ran" or "the write went to a name that does
    /// not exist".
    diagnostics: bool,
    /// Control animations (fly-in, fade, pulse, …) — the same effects the
    /// designer preview shows, on every host.
    anim: cobolt_forms::anim::AnimRuntime,
    /// One-shot guard for the load-time (`OnFormLoad` / `OnShow`) animations.
    anim_started: bool,
    /// Previous frame's timestamp — the animation clock's delta.
    last_frame: Option<std::time::Instant>,
    /// Controls the pointer was inside last frame, so `OnHover` animations fire
    /// on entry only. Hover/click triggers are derived from the rendered rects
    /// rather than from `RenderOutput::events`, which the engine emits only for
    /// events that have a bound COBOL handler.
    hovered: std::collections::HashSet<String>,
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
    /// Broadcasts closed handles back to the interpreter (R24 NULLing).
    closed_tx: mpsc::Sender<String>,
    /// The form's object name (UPPER) — receiver of form-level events.
    form_object: String,
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
}

impl FormHost {
    fn send_event(&mut self, ev: FormEvent) {
        if self.ev_tx.send(ev).is_ok() {
            self.pending.fetch_add(1, Ordering::Relaxed);
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
                        handle, form_id, ..
                    } => {
                        // Real child viewports land with the multi-viewport
                        // host (T1 spike gating). Until then the spawn is
                        // released immediately so callers never deadlock:
                        // the handle NULLs, a held modal reply resolves.
                        eprintln!(
                            "form-host: OpenForm(\"{form_id}\") accepted, but child windows \
                             are not hosted yet — releasing {handle} (multi-window host \
                             pending)"
                        );
                        next.extend(self.supervisor.form_finished(&handle));
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
                                    let _ = self.ev_tx.send(FormEvent::quit());
                                }
                                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                            }
                        }
                    }
                    HostAction::FocusWindow { handle } => {
                        if handle == ROOT_HANDLE {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                        }
                    }
                    HostAction::SetWindowState { handle, state } => {
                        if handle == ROOT_HANDLE {
                            let s = state.trim();
                            if s.eq_ignore_ascii_case("Minimized") {
                                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                            } else if s.eq_ignore_ascii_case("Maximized") {
                                ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(true));
                            } else {
                                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                                ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(false));
                            }
                        }
                    }
                    HostAction::SetFullScreen { handle, on } => {
                        if handle == ROOT_HANDLE {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(on));
                        }
                    }
                    HostAction::SetTitleVisible { handle, on } => {
                        if handle == ROOT_HANDLE {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Decorations(on));
                        }
                    }
                    HostAction::NotifyCloseRejected { handle } => {
                        if handle == ROOT_HANDLE {
                            let form = self.form_object.clone();
                            self.send_event(FormEvent::new(form, "onCloseRejected"));
                        }
                    }
                    HostAction::NotifyClosed { handle } => {
                        let _ = self.closed_tx.send(handle);
                    }
                    HostAction::Exit => {
                        if !self.quit_sent {
                            self.quit_sent = true;
                            let _ = self.ev_tx.send(FormEvent::quit());
                        }
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                }
            }
            work = next;
        }
    }

    /// Opt-in diagnostic (`COBOLT_DATABIND_TRACE=1`). For each repeating-group
    /// member of instance 1, write the exact id the renderer looks up and whether
    /// that id is present in `state` byte-exact vs. case-insensitively. A CI-only
    /// hit means the value landed under a differently-cased key than the render
    /// draws with — the classic run-form databind blank. Written once to
    /// `cobolt-databind-render.log` in the platform's diagnostics directory.
    fn dump_databind_trace(&self) {
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

    /// Resolve a control id arriving from COBOL (upper-cased by the compiler)
    /// to the designer's original-case state key — otherwise a handler's
    /// property writes land in an orphan "LABEL-1" entry the renderer (which
    /// looks up by the designed "Label-1" id) never reads.
    fn resolve_ctrl_key(&self, id: &str) -> String {
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

    /// See [`crate::state::state_entry_mut`].
    fn state_entry_mut(&mut self, key: &str) -> &mut CtrlState {
        state_entry_mut(&mut self.state, &self.controls, key)
    }

    /// For a repeating-group member id (case-insensitive), return its
    /// original-case id and the id of its repeating-GroupBox ancestor.
    fn array_member_group(&self, ctrl_id: &str) -> Option<(String, String)> {
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
}

impl FormHost {
    /// The form's background, resolved once and shared by the live render and
    /// by the static face the window effects animate — so an entrance reveals
    /// the form WITH its gradient / background image instead of jumping to it
    /// when the animation ends.
    fn backdrop(&self, ctx: &egui::Context) -> cobolt_forms::render::Backdrop {
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

    /// 038 — paint one effect frame: the form's STATIC face (background +
    /// every visible control via the shared `draw_control` pipeline, scaled
    /// into whatever geometry the effect chooses) transformed by progress
    /// `t`. Pixel parity with the designer comes free — same painter.
    fn paint_fx_frame(
        &self,
        root_ui: &egui::Ui,
        effect: cobolt_forms::window_fx::WindowEffect,
        duration_ms: u32,
        t: f32,
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
        let bg = cobolt_forms::render::backdrop_color(&self.bg_hex, self.transparency);
        let backdrop = self.backdrop(root_ui.ctx());
        let controls = &self.controls;
        let time = self.start.elapsed().as_secs_f64();
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
            &mut |p, target| Self::paint_face(p, target, rect, controls, &backdrop),
        );
    }

    /// The static face the effects animate: exactly the picture the live UI
    /// hands back — the full backdrop (colour, gradient, theme art or image)
    /// stretched over the window, and every visible control at its DESIGNED
    /// size — mapped from the untransformed `base` rect into whatever
    /// geometry the effect chose. Scaling the controls against the form size
    /// instead would blow them up on a window bigger than the form and snap
    /// them back the moment the animation ended.
    fn paint_face(
        painter: &egui::Painter,
        target: egui::Rect,
        base: egui::Rect,
        controls: &[cobolt_forms::Control],
        backdrop: &cobolt_forms::render::Backdrop,
    ) {
        cobolt_forms::render::paint_backdrop(painter, target, backdrop);
        let sx = target.width() / base.width().max(1.0);
        let sy = target.height() / base.height().max(1.0);
        for c in controls.iter().filter(|c| c.visible) {
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
            cobolt_forms::render::backdrop_color(&self.bg_hex, 0).to_normalized_gamma_f32()
        }
    }

    fn ui(&mut self, root_ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.ui_impl(root_ui);
    }
}

impl FormHost {
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
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
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
                    ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(x, y)));
                }
                self.pending_start_position = None;
            }
        }
        // Theme pack + glass style for the unified painter (per frame — same
        // contract every host follows).
        cobolt_forms::paint::set_active_theme(ctx, self.theme_pack.clone());
        cobolt_forms::paint::set_glass_style(ctx, self.glass_style);
        cobolt_forms::paint::set_surface_style(ctx, self.surface_style);

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
        // unchanged.
        if self.surface_style == cobolt_forms::paint::SurfaceStyle::Elegance {
            cobolt_forms::paint::install_elegance_theme(ctx);
        }

        // The per-host seam (R30): e.g. the compiled application replays its
        // EXEC RUST block windows here, every frame — that is what
        // `show_viewport_deferred` requires (miss a frame and the window
        // closes). A block cannot do this itself: it runs once, off-thread.
        self.hooks.per_frame(ctx);

        // Program ended (STOP RUN / runtime error) → close the window — via
        // the exit effect when one is configured (038 R10, plan D6: one close
        // choreography regardless of why the window closes).
        if self.finished.load(Ordering::Relaxed) {
            if self.fx_exit.is_active() && self.fx_exit_start.is_none() {
                self.fx_exit_start = Some(std::time::Instant::now());
            }
            if self.fx_exit_start.is_none() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
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
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
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
                ctx.send_viewport_cmd(egui::ViewportCommand::Decorations(false));
            }
            let exit_ms = fx_duration_ms(&self.fx_exit, root_ui.max_rect().width());
            let dur = exit_ms.max(1) as f64 / 1000.0;
            let t_lin = 1.0 - (started.elapsed().as_secs_f64() / dur).min(1.0);
            if t_lin <= 0.0 {
                if !self.quit_sent {
                    self.quit_sent = true;
                    let _ = self.ev_tx.send(FormEvent::quit());
                }
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            } else {
                let t = self
                    .fx_exit
                    .effect
                    .progress(self.fx_exit.easing, t_lin as f32);
                self.paint_fx_frame(root_ui, self.fx_exit.effect, exit_ms, t);
                ctx.request_repaint();
            }
            return;
        }

        // 037 R14 — onFullScreenChanged fires on ACTUAL transitions only,
        // read back from the viewport (the OS may refuse a request). The
        // live value is mirrored onto the form object first so the handler
        // reads the new state.
        let fs = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
        if fs != self.fullscreen_actual {
            self.fullscreen_actual = fs;
            let _ = self.input_tx.send(StateUpdate {
                ctrl_id: self.form_object.clone(),
                prop: "FullScreen".into(),
                value: if fs { "1".into() } else { "0".into() },
                instance_index: 0,
            });
            let form = self.form_object.clone();
            self.send_event(FormEvent::new(form, "onFullScreenChanged"));
        }

        // 038 R9 — restore-after-minimize replays the ENTRANCE visuals only:
        // no form events, no control-animation replay (`anim_started` stays
        // true). Edge-triggered on the observed minimized transition.
        let minimized = ctx.input(|i| i.viewport().minimized.unwrap_or(false));
        if minimized != self.minimized_actual {
            let was = self.minimized_actual;
            self.minimized_actual = minimized;
            if was && !minimized && self.fx_restore && self.fx_entrance.is_active() {
                self.fx_entrance_done = false;
                self.fx_entrance_start = None;
            }
        }

        // ── Animation clock ──────────────────────────────────────────────────
        // Load-time animations start once the WINDOW has fully materialised:
        // the entrance effect completes first, then the controls come alive
        // (038 R8). Without an entrance the gate opens on the first frame,
        // exactly as before.
        if !self.anim_started && self.fx_entrance_done {
            self.anim_started = true;
            self.anim.start_form_load(&self.controls);
        }
        let now = std::time::Instant::now();
        let dt = self
            .last_frame
            .map(|t| now.duration_since(t).as_secs_f32())
            .unwrap_or(0.0);
        self.last_frame = Some(now);
        let animating = self.anim.tick(dt);

        // Apply property changes coming from the COBOL interpreter. Route each
        // update to the designer-case state key (COBOL upper-cases ids), and
        // repeating-group member writes to the drawn card-instance id.
        let mut drained = 0usize;
        while let Ok(u) = self.state_rx.try_recv() {
            // 037 R16 — mirror the form object's FormState into the
            // supervisor so close vetoes see the live value.
            if u.prop.eq_ignore_ascii_case("FormState")
                && u.ctrl_id.eq_ignore_ascii_case(&self.form_object)
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
                        self.anim
                            .play_programmatic(&self.controls, &u.ctrl_id, &u.value)
                    }
                    AnimCommand::Stop => self.anim.stop_all(&u.ctrl_id),
                    AnimCommand::Pause => self.anim.pause_all(&u.ctrl_id),
                }
                drained += 1;
                continue;
            }
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
            // R27 — the live trace: which designed control this write landed
            // on, or NO SUCH CONTROL with the ids that do exist. The routing
            // itself is unchanged; the trace only reports it.
            if self.diagnostics {
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
            self.state_entry_mut(&key).set(&u.prop, u.value);
            drained += 1;
        }

        // ── One-shot databind diagnostic (opt-in) ────────────────────────────
        // COBOLT_DATABIND_TRACE=1 (also true/on) writes, once, the mismatch
        // between the state keys the interpreter populated and the instanced ids
        // the renderer will look up for each repeating-group member. Decisive for
        // "cards show designed defaults in run-form but not in preview". The IDE
        // sets this from the project's Data-bind trace setting — always (incl.
        // "0"), so test the value rather than mere presence.
        if !self.db_dumped
            && crate::diagnostics::databind_trace_enabled()
            && self.state.keys().any(|k| k.contains('.'))
        {
            self.db_dumped = true;
            self.dump_databind_trace();
        }

        // DISPLAY output → stdout (the IDE pipes this into its Output pane).
        // Explicit flush: stdout is BLOCK-buffered when piped, so without it
        // DISPLAY lines sit in the buffer instead of reaching the reader live.
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

        // Ignore input for a brief warm-up after the window appears.
        let armed = self.start.elapsed().as_millis() > 450;

        // Form-level lifecycle: onShow / onActivate fire once after warm-up
        // (unknown events are ignored by the generated dispatch loop, so this
        // is safe for any form).
        if armed && !self.lifecycle_sent {
            self.lifecycle_sent = true;
            let name = self.form_name.clone();
            self.send_event(FormEvent::new(&name, "onShow"));
            self.send_event(FormEvent::new(&name, "onActivate"));
        }

        let bg_fill = cobolt_forms::render::backdrop_color(&self.bg_hex, self.transparency);
        let form_size = self.form_size;

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
                // The window wears its chrome again, arriving together with
                // the finished form (038 — the title bar was off so nothing
                // stood still while the effect played).
                if self.fx_chrome_pending {
                    self.fx_chrome_pending = false;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Decorations(true));
                }
            } else {
                let t = self
                    .fx_entrance
                    .effect
                    .progress(self.fx_entrance.easing, t_lin as f32);
                self.paint_fx_frame(root_ui, self.fx_entrance.effect, ent_ms, t);
                ctx.request_repaint();
                return;
            }
        }

        // Render the whole form through the unified engine (one renderer for
        // the designer, preview, and every host — spec 017).
        let output = {
            let controls = self.controls.clone();
            let st = LiveState {
                state: &self.state,
                anim: &self.anim,
            };
            let active_tabs = cobolt_forms::containers::ActiveTabs::default();
            let backdrop = self.backdrop(ctx);
            let mut out = cobolt_forms::render::RenderOutput::default();
            // On a see-through window the panel must NOT fill: the engine
            // paints the same backdrop across the whole window a moment
            // later, and painting a translucent colour twice would double the
            // form's designed opacity against the desktop.
            let panel_fill = if self.fx_transparent {
                egui::Color32::TRANSPARENT
            } else {
                bg_fill
            };
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE.fill(panel_fill))
                .show(root_ui, |ui| {
                    // Floating scrollbars overlay the content instead of
                    // reserving a gutter, so no light track strip shows on the
                    // right/bottom edges when the form fits (only appears, as an
                    // overlay, if the user shrinks the resizable window).
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
                // Pointer events are already covered by the rect pass above —
                // taking them from here too would restart the same animation twice.
                if ev.event.eq_ignore_ascii_case("onClick")
                    || ev.event.eq_ignore_ascii_case("onHoverEnter")
                {
                    continue;
                }
                self.anim.fire_event(&self.controls, &ev.ctrl_id, &ev.event);
            }
        }

        // Apply value updates locally, sync them to the interpreter (so
        // handlers read the live value), and forward UI events — once armed.
        let mut interacted = false;
        if armed {
            for (id, key, val) in &output.prop_updates {
                self.state_entry_mut(id).set(key, val.clone());
                let _ = self
                    .input_tx
                    .send(StateUpdate::new(id.clone(), key.clone(), val.clone()));
                interacted = true;
            }
            // Coalesce timer ticks against a still-queued backlog (WinForms
            // semantics) so a slow handler can't flood the event queue. User
            // events (clicks, edits, focus, quit) are never dropped.
            let backlog = self.pending.load(Ordering::Relaxed) > 0;
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
                self.send_event(FormEvent::new(dispatch_id, ev.event).with_index(inst));
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
                .controls
                .iter()
                .filter(|c| matches!(c.control_type, cobolt_forms::ControlType::FileDropZone))
                .map(|c| c.id.clone())
                .collect();
            for id in file_drop_zone_ids {
                let key = format!("filedropzone:{id}");
                if let Some(Some(path)) = crate::file_dialog::take(&key) {
                    let val = path.display().to_string();
                    self.state_entry_mut(&id).set("DroppedFiles", val.clone());
                    let _ = self.input_tx.send(StateUpdate::new(
                        id.clone(),
                        "DroppedFiles".to_owned(),
                        val,
                    ));
                    self.send_event(FormEvent::new(id, "onFilesDropped".to_owned()));
                    interacted = true;
                }
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
            || self.anim.is_animating()
            || self.pending.load(Ordering::Relaxed) > 0;
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
            fx_entrance: FxSpec::parse(entrance),
            fx_exit: FxSpec::parse(exit),
            fx_restore: restore,
            theme_pack: None,
            surface_style: cobolt_forms::paint::SurfaceStyle::LiquidGlass,
            icon_path: None,
            title_fallback: String::new(),
            hooks: Box::new(NoHooks),
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
            !app.anim_started,
            "load animations are gated behind the entrance (038 R8)"
        );

        // Force the playback clock past the end: the next frame completes the
        // entrance and restores the chrome. The animation gate sits ABOVE the
        // playback block in the frame, so it opens on the FOLLOWING frame —
        // the same one-frame handover the run-form host always had.
        app.fx_entrance_start = Some(Instant::now() - Duration::from_millis(3200));
        let cmds = frame(&mut app, &ctx, raw());
        assert!(app.fx_entrance_done, "entrance completes past its duration");
        assert!(
            cmds.iter()
                .any(|c| matches!(c, egui::ViewportCommand::Decorations(true))),
            "the designed chrome comes back with the finished form; got {cmds:?}"
        );
        frame(&mut app, &ctx, raw());
        assert!(
            app.anim_started,
            "load animations start on the first live-UI frame (038 R8)"
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
        assert!(app.anim_started, "no entrance ⇒ load animations start at once");
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
        app.anim_started = true;
        app.lifecycle_sent = true;
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
        assert!(app.anim_started, "control animations do NOT replay");
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

        app.start = Instant::now() - Duration::from_millis(600);
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
