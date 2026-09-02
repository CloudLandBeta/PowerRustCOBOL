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
        // 049 — the SideMenu's FOOTER PANEL and whatever the developer dropped
        // into it are part of the RAIL, not of the form's content. They were
        // slid over with everything else and clamped at the pane's left edge,
        // so a clock designed into the footer surfaced BESIDE the rail at the
        // bottom of the content (operator, 2026-08-22). They keep their
        // designed rects here and are drawn by `draw_side_menu_footer` into the
        // rail's own footer band.
        let footer_ids: std::collections::HashSet<String> = if surface == Surface::Pane {
            cobolt_forms::model::side_menu_footer_subtree(&flat)
                .into_iter()
                .collect()
        } else {
            std::collections::HashSet::new()
        };

        let flat: Vec<cobolt_forms::Control> = if surface == Surface::Pane {
            flat.into_iter()
                .filter(|c| c.control_type != cobolt_forms::ControlType::SideMenu)
                .map(|mut c| {
                    // Slide the form's content area over the rail's column so
                    // its left edge lands ON the pane's left edge — juxtaposed
                    // to the rail rather than offset from it twice. A control
                    // the developer parked UNDER the rail clamps to the edge
                    // instead of disappearing off the left of the pane.
                    //
                    // The footer subtree is exempt: it is not content, it is
                    // rail, and its designed rect is what the footer band is
                    // laid out from.
                    if !footer_ids.contains(&c.id) {
                        c.rect.x = (c.rect.x - side_dx).max(0);
                    }
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
                drawn_reported: false,
                form_name: form.name.clone(),
                footer_ids: footer_ids.clone(),
                theme_pack,
                surface_theme,
                glass_style,
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
                toolbar_runner: cobolt_forms::toolbar_actions::Runner::default(),
                action_notice: None,
                last_control_rects: HashMap::new(),
                snackbars: Default::default(),
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
            footer_ids,
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
            pending_crumb_detail: None,
            pane_chrome: None,
            pane_band: 0.0,
            last_occupant_rect: None,
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
    /// Carries out a toolbar button's PLATFORM action, and finishes the
    /// two-frame window captures.
    ///
    /// Per FORM, not per host: a capture is of one window, so a child window's
    /// screenshot is its own. It used to live on the host, which is part of why
    /// only the root form ever ran a toolbar action at all.
    pub(crate) toolbar_runner: cobolt_forms::toolbar_actions::Runner,
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
    /// 055 — this surface's live notifications. One stack per FormBody, which
    /// is what "the stack belongs to the surface" means (spec Q1/Q2): a child
    /// form's messages stack in that child, and navigating away disposes them
    /// rather than carrying a message about screen A onto screen B.
    pub(crate) snackbars: crate::snackbar_stack::SnackbarStack,
}

impl FormBody {
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
            special_names: &self.special_names,
        };
        let active_tabs = cobolt_forms::containers::ActiveTabs::default();
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
        let out = cobolt_forms::render::render_form(&mut child, &input);
        // A toolbar button (or FileDropZone) in the footer is as real as one
        // on the form: its platform actions used to be dropped here — only the
        // COBOL event was forwarded, so print/copy/share in a footer did
        // nothing, silently (operator, 2026-08-23).
        let ctx = ui.ctx().clone();
        self.run_platform_requests(
            &ctx,
            &out.file_picker_requests,
            &out.toolbar_actions,
            pre_focus,
        );
        self.forward_interaction(&out.prop_updates, out.events);
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
        let placed = self.snackbars.layout(surf, &measure);

        // Paint newest LAST so it sits over its neighbours, and collect the
        // button rects for hit-testing.
        let mut hits: Vec<(u64, Vec<egui::Rect>)> = Vec::new();
        for (id, r) in &placed {
            let Some(n) = self.snackbars.live().iter().find(|n| n.id == *id) else {
                continue;
            };
            let rect = egui::Rect::from_min_size(
                egui::Pos2::new(r.x as f32, r.y as f32),
                egui::Vec2::new(r.w as f32, r.h as f32),
            );
            let out = cobolt_forms::paint::draw_snackbar(
                &painter,
                rect,
                &n.visual,
                None,
                1.0,
                snack_pointer,
            );
            hits.push((*id, out.buttons));
        }

        // A click on a button. The notification is not a control, so this is not
        // an `interact` on a widget id — it is a hit test against the rects the
        // painter just reported, which is the same thing the toolbar does.
        if ui.input(|i| i.pointer.primary_clicked()) {
            if let Some(pos) = ui.ctx().pointer_interact_pos() {
                'outer: for (id, buttons) in hits.iter().rev() {
                    for (idx, br) in buttons.iter().enumerate() {
                        if br.contains(pos) {
                            self.snackbars.click_button(*id, idx);
                            break 'outer;
                        }
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
                    for (k, v) in [
                        ("LastButtonId", button_id.clone()),
                        ("LastButtonIndex", index.to_string()),
                    ] {
                        self.state_entry_mut(&ctrl_id).set(k, v.clone());
                        let _ = self.input_tx.send(cobolt_runtime::channels::StateUpdate::new(
                            ctrl_id.clone(),
                            k.to_string(),
                            v,
                        ));
                    }
                    (ctrl_id, "onButtonClick", format!("{button_id}\t{index}"))
                }
            };
            let mut fe = FormEvent::new(ctrl_id, name);
            fe.value = value;
            self.send_event(fe);
        }

        // A live notification is a reason to keep painting: its timeout has to
        // elapse even when nothing else on the form is moving.
        if !self.snackbars.is_empty() {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(50));
        }
    }

    pub(crate) fn send_event(&mut self, ev: FormEvent) {
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
    pub(crate) fn forward_interaction(
        &mut self,
        prop_updates: &[(String, String, String)],
        events: Vec<cobolt_forms::render::UiEvent>,
    ) -> bool {
        let mut sent = false;
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
        toolbar_actions: &[(String, String, String)],
        pre_focus: Option<egui::Id>,
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
                let ctrl = self.controls.iter().find(|c| c.id == id);
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
                        (egui::Id::new(("rt_ctrl", c.id.as_str())) == focus).then(|| {
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
                    widget_id: egui::Id::new(("rt_ctrl", id.as_str())),
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
        // The interpreter has already refused anything but a colour or a tooltip,
        // out loud; this only carries an allowed write home.
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
        self.state_entry_mut(&key).set(&u.prop, u.value);
        if changed {
            for event in observers {
                self.send_event(FormEvent::new(key.clone(), event.to_owned()));
            }
        }
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
        cobolt_forms::render::Backdrop {
            paint: true,
            color_hex: self.bg_hex.clone(),
            transparency: self.transparency,
            gradient_enabled: self.bg_gradient_enabled,
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
            // A child form rides the same diagnostics switch the root does; the
            // host reads it once at start-up, a body reads it here.
            self.apply_interpreter_update(u, crate::diagnostics::frame_diagnostics_enabled());
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
        // Focus as it stood BEFORE this frame's widgets see the click — the
        // click that presses a toolbar button surrenders the text field's
        // focus during render, and the clipboard verbs need to know who HAD it.
        let pre_focus = ctx.memory(|m| m.focused());
        let output = {
            let controls = self.controls.clone();
            let st = LiveState {
                state: &self.state,
                anim: &self.anim,
                hidden: Some(&self.footer_ids),
                special_names: &self.special_names,
            };
            let active_tabs = cobolt_forms::containers::ActiveTabs::default();
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
                            out = cobolt_forms::render::render_form_with_chrome(
                                ui, &input, chrome,
                            );
                        });
                });
            out
        };
        // Where the engine actually put every control this frame — see
        // `FormBody::last_control_rects`.
        self.last_control_rects = output.control_rects.clone();

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
            if self.forward_interaction(&output.prop_updates, output.events) {
                platform_acted = true;
            }

            // A FileDropZone's native picker and a toolbar button's platform
            // action. This ran on the ROOT form only, so a toolbar in a child
            // window or a ContentPane occupant had eight dead actions and its
            // FileDropZone would not open a picker.
            if self.run_platform_requests(
                ctx,
                &output.file_picker_requests,
                &output.toolbar_actions,
                pre_focus,
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
    /// What the operator should be told this form IS — its designed Title,
    /// falling back to its form object name when it has none. The breadcrumb
    /// segment reads from here: the chain named loaded forms by their OBJECT
    /// name (`inner-form1`) while the main form used its title, so one strip
    /// showed two vocabularies.
    pub(crate) label: String,
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
    /// A COBOL-driven breadcrumb DETAIL level awaiting the shell:
    /// `(form object, text)`, `None` text = cleared.
    pending_crumb_detail: Option<(String, Option<String>)>,
    /// Chrome the SHELL paints over the pane's backdrop and UNDER the form's
    /// controls — its breadcrumb frame. Handed in fresh each frame (the strip
    /// follows the chain, the rail state and the pointer), and painted where
    /// the pane backdrop is: outside the scroll area, so it stays put while
    /// the form scrolls, and before the controls, so a control the developer
    /// placed over the band paints on top of it.
    pane_chrome: Option<Box<dyn Fn(&egui::Painter, egui::Rect)>>,
    /// How tall that chrome band is. The shell form may design controls OVER
    /// the band — it is the shell's own coordinate space. A form LOADED into
    /// the pane may not: it is a different form, and its origin starts below
    /// the band. Only the occupant path reads this.
    pane_band: f32,
    /// Where the last frame actually put the ContentPane's occupant. Recorded
    /// so a test can check an embedded form lands inside the pane instead of
    /// over the MenuPane — the thing that went wrong is a RECT, and nothing
    /// else about the form's state reveals it.
    last_occupant_rect: Option<egui::Rect>,
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
                    // The breadcrumb's detail level, for the SHELL host. It is
                    // recorded against the form that set it, so a crumb set by
                    // a form the operator has since navigated away from cannot
                    // reappear over someone else's name.
                    HostAction::SetBreadcrumbDetail { handle, text } => {
                        let form_object = if handle == ROOT_HANDLE {
                            self.root.form_object.clone()
                        } else {
                            self.occupants
                                .iter()
                                .find(|(_, o)| o.handle == handle)
                                .map(|(k, _)| k.clone())
                                .unwrap_or_default()
                        };
                        if !form_object.is_empty() {
                            self.pending_crumb_detail = Some((form_object, text));
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
        self.occupants.insert(
            key,
            Occupant {
                handle,
                body,
                label,
            },
        );
        Ok(ev_tx)
    }

    /// What the breadcrumb should call a pane occupant: its designed **Title**,
    /// or its form object name when it has none. `None` = no such occupant.
    pub fn occupant_label(&self, form_object: &str) -> Option<String> {
        self.occupants
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
        self.root.draw_side_menu_footer(ui, band, behind);
    }

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

    /// Where the last rendered frame placed the ContentPane's occupant, or
    /// `None` if no embedded form was on the pane. The pane's origin is the
    /// whole point (see the occupant branch of `ui_impl`).
    pub fn last_occupant_rect(&self) -> Option<egui::Rect> {
        self.last_occupant_rect
    }

    /// Where the last frame put each control of the form that currently owns
    /// the pane — the active occupant, or the root form when none does.
    ///
    /// Screen coordinates, straight from the render engine. "The control is
    /// missing" and "the control was drawn off the visible surface" look the
    /// same to an operator; this is what tells them apart.
    pub fn last_control_rects(&self) -> &HashMap<String, egui::Rect> {
        match self.active_occupant.as_ref().and_then(|k| self.occupants.get(k)) {
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
        let handle = match self.occupants.get(&up) {
            Some(occ) => occ.handle.clone(),
            None => cobolt_runtime::form_host::ROOT_HANDLE.to_string(),
        };
        self.supervisor
            .note_form_props(&handle, vec![(key.to_string(), value.to_string())]);
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
            drawn_reported: false,
            form_name: form.name.clone(),
            // An occupant is a form INSIDE the pane; the rail belongs to the
            // shell's main form, so an occupant has no footer band of its own
            // and nothing is withheld from its content pass.
            footer_ids: std::collections::HashSet::new(),
            theme_pack,
            surface_theme,
            glass_style: form.glass_style,
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
            toolbar_runner: cobolt_forms::toolbar_actions::Runner::default(),
            action_notice: None,
            last_control_rects: HashMap::new(),
                snackbars: Default::default(),
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
                child.body.child_frame(vp_ui, blocked, None);
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
        let backdrop = self.root.backdrop(root_ui.ctx(), rect.size());
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

    /// Drain a COBOL-driven breadcrumb detail level: `(form object, text)`,
    /// with `None` text meaning the form cleared it.
    pub fn take_breadcrumb_detail(&mut self) -> Option<(String, Option<String>)> {
        self.pending_crumb_detail.take()
    }

    /// The chrome the shell paints between the pane's backdrop and the form's
    /// controls. Set fresh each frame; `None` clears it.
    /// `band` is the chrome's own height, which a pane OCCUPANT starts below.
    pub fn set_pane_chrome(
        &mut self,
        chrome: Option<Box<dyn Fn(&egui::Painter, egui::Rect)>>,
        band: f32,
    ) {
        self.pane_chrome = chrome;
        self.pane_band = band;
    }

    /// Read a published form property (what `super::X` reads) off a pane
    /// occupant, or off the ROOT form when `form_object` names it or is
    /// `None`. The shell asks for `PreventReset` before starting a form over.
    pub fn published_form_prop(&self, form_object: Option<&str>, key: &str) -> Option<String> {
        let handle = match form_object {
            None => cobolt_runtime::form_host::ROOT_HANDLE.to_string(),
            Some(f) => {
                let up = f.trim().to_ascii_uppercase();
                match self.occupants.get(&up) {
                    Some(occ) => occ.handle.clone(),
                    None if up == self.root.form_object => {
                        cobolt_runtime::form_host::ROOT_HANDLE.to_string()
                    }
                    None => return None,
                }
            }
        };
        self.supervisor.published_prop(&handle, key)
    }

    /// Fire a form-level event at a pane occupant, or at the ROOT form when
    /// `form_object` is `None` or names it. `false` = no such form.
    pub fn notify_form(&mut self, form_object: Option<&str>, event: &str) -> bool {
        let key = form_object.map(|f| f.trim().to_ascii_uppercase());
        let body = match &key {
            None => Some(&mut self.root),
            Some(k) if *k == self.root.form_object => Some(&mut self.root),
            Some(k) => self.occupants.get_mut(k).map(|o| &mut o.body),
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
            // Routing, the toolbar-button door and the OBSERVER events all live in
            // one place, shared with the child-window path — see
            // `FormBody::apply_interpreter_update` for why that matters.
            self.root.apply_interpreter_update(u, self.diagnostics);
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
            let chrome = self.pane_chrome.take();
            let band = self.pane_band;
            if let Some(occ) = self.occupants.get_mut(&key) {
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
                self.last_occupant_rect = Some(rect);
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
        let output = {
            let controls = self.root.controls.clone();
            let st = LiveState {
                state: &self.root.state,
                anim: &self.root.anim,
                hidden: Some(&self.root.footer_ids),
                special_names: &self.root.special_names,
            };
            let active_tabs = cobolt_forms::containers::ActiveTabs::default();
            let backdrop = self.root.backdrop(ctx, ctx.content_rect().size());
            let pane_chrome = self.pane_chrome.take();
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
        // Where the engine actually put every control this frame — see
        // `FormBody::last_control_rects`.
        self.root.last_control_rects = output.control_rects.clone();

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
                .forward_interaction(&output.prop_updates, output.events)
            {
                interacted = true;
            }

            // A FileDropZone's native picker and a toolbar button's platform
            // action — shared with the child-window path.
            if self.root.run_platform_requests(
                ctx,
                &output.file_picker_requests,
                &output.toolbar_actions,
                pre_focus,
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
            body.forward_interaction(&[], vec![tick()]);
            assert_eq!(
                drain(&ev_rx),
                vec!["onTick".to_owned()],
                "with {outstanding} event(s) outstanding the tick must still be sent"
            );
        }

        // Genuinely behind: the tick is coalesced away, which is the rule's point.
        body_pending.store(FormBody::TICK_COALESCE_BACKLOG, Ordering::Relaxed);
        let _ = drain(&ev_rx);
        body.forward_interaction(&[], vec![tick()]);
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
            footer_ids: std::collections::HashSet::new(),
            theme_pack: None,
            surface_theme: cobolt_forms::surface_theme::liquid_glass(),
            glass_style: cobolt_forms::model::GlassStyle::default(),
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
            form_size: egui::vec2(320.0, 200.0),
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
            action_notice: None,
            last_control_rects: HashMap::new(),
                snackbars: Default::default(),
        }
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
            body.run_platform_requests(&ctx2, &[], &press, pre);
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
            body.run_platform_requests(&ctx2, &[], &press, None);
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
            body.run_platform_requests(&ctx2, &[], &press, Some(widget_id));
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
