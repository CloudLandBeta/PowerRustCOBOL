// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The application shell (spec 049): ONE window divided into a MenuPane, a
//! breadcrumb strip and a ContentPane.
//!
//! Layout rules this module owns:
//! - **R4** — exactly three regions.
//! - **R8** — the MenuPane is Open or Collapsed (two fixed widths; Collapsed
//!   is a narrow icon rail).
//! - **R37/R40** — each pane scrolls in its OWN [`egui::ScrollArea`] with a
//!   distinct id, so independence is structural: there is no shared scroll
//!   state to keep in sync.
//! - **R38** — the MenuPane's width never follows a window/content resize:
//!   the panel is `resizable(false)` at an exact size, and a size change is
//!   absorbed entirely by the ContentPane.
//! - **R14** — the breadcrumb is chrome OUTSIDE the ContentPane, painted by
//!   the shell, so a loaded form's background can never affect it.

use egui::{Rect, Ui, Vec2};

/// Default Open width of the MenuPane, in points.
pub const MENU_PANE_OPEN_WIDTH: f32 = 220.0;
/// Default Collapsed width — a narrow icon rail that keeps the root items
/// reachable (spec R8).
pub const MENU_PANE_COLLAPSED_WIDTH: f32 = 48.0;
/// Height of the breadcrumb strip.
pub const BREADCRUMB_HEIGHT: f32 = 28.0;

/// The glyph on the MenuPane's Open/Collapsed toggle. It sits on the pane
/// itself, above the mounted menus, and is drawn whether or not a single menu
/// item exists — collapsing the sidebar is the operator's control over the
/// window, never a function of what the developer put in the menu.
pub const MENU_PANE_TOGGLE: &str = "☰";

/// The default chrome fill — an EXPLICIT, fully opaque paint. In a
/// transparent shell window (R43) an unpainted region is a hole to the
/// desktop, so the MenuPane and breadcrumb always paint; only the
/// ContentPane's form backdrop may carry alpha.
pub const CHROME_FILL: egui::Color32 = cobolt_forms::breadcrumb::CHROME;

/// Source-over composite, through the engine's own helper so the shell, the
/// canvas and the preview resolve a translucent colour identically. The shell
/// needs it because the rail's designed `BackgroundColor` is routinely
/// TRANSLUCENT: on the designer canvas it composites over the form it sits on,
/// and the shell has to reproduce that rather than painting the bare colour
/// onto a transparent window.
fn over(src: egui::Color32, base: egui::Color32) -> egui::Color32 {
    cobolt_forms::paint::composite_premultiplied_over(src, base)
}

// ── MenuPane state persistence (R9) ─────────────────────────────────────────
//
// A per-APPLICATION user preference — every shipped binary keeps its own,
// following the `<data_dir>/cobolt/…` convention the IDE's ui_prefs uses, but
// keyed by the application name. Deliberately std-only: one key, one line.

/// Where an application's shell state lives:
/// `<data_dir>/cobolt/apps/<app>/shell.toml`.
pub fn shell_state_path(app_name: &str) -> Option<std::path::PathBuf> {
    let safe: String = app_name
        .trim()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' })
        .collect();
    let base = std::env::var_os("HOME").map(std::path::PathBuf::from)?;
    #[cfg(target_os = "macos")]
    let data = base.join("Library/Application Support");
    #[cfg(not(target_os = "macos"))]
    let data = base.join(".local/share");
    Some(data.join("cobolt").join("apps").join(safe).join("shell.toml"))
}

/// Persist the MenuPane state (R9). `path` is [`shell_state_path`]'s answer —
/// parameterised so tests use a temp dir.
pub fn save_collapsed_to(path: &std::path::Path, collapsed: bool) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, format!("collapsed = {collapsed}\n"))
}

/// Read the persisted MenuPane state. Absent / unreadable ⇒ `None` (the shell
/// defaults to Open).
pub fn load_collapsed_from(path: &std::path::Path) -> Option<bool> {
    let text = std::fs::read_to_string(path).ok()?;
    for line in text.lines() {
        let line = line.trim();
        if let Some(v) = line.strip_prefix("collapsed") {
            let v = v.trim_start().strip_prefix('=')?.trim();
            return match v {
                "true" => Some(true),
                "false" => Some(false),
                _ => None,
            };
        }
    }
    None
}

/// What one shell frame laid out — the geometry and scroll state the caller
/// (and the tests) can reason about.
#[derive(Debug, Clone, PartialEq)]
pub struct ShellLayout {
    pub menu_rect: Rect,
    pub breadcrumb_rect: Rect,
    pub content_rect: Rect,
    /// The MenuPane's own scroll offset.
    pub menu_scroll: Vec2,
    /// The ContentPane's own scroll offset.
    pub content_scroll: Vec2,
}

/// Which MenuPane slot a mounted menu (or a click) belongs to (R6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuSlot {
    /// The main form's menu — mounted once, never replaced (R6).
    Root,
    /// The current subsystem's menu — replaced WHOLESALE on entry (R7).
    Contextual,
}

/// A menu mounted into a MenuPane slot.
#[derive(Debug, Clone)]
pub struct MountedMenu {
    /// The form object whose menu this is (its handlers stay live while the
    /// menu is mounted — residency is the navigation chain's business).
    pub form_object: String,
    pub def: cobolt_forms::menu::MenuDefinition,
}

/// One activated menu item, drained by the navigation layer.
#[derive(Debug, Clone, PartialEq)]
pub struct MenuClick {
    pub slot: MenuSlot,
    pub item_id: String,
    /// The item's action verbatim (`open-form:<NAME>`, `event`, …).
    pub action: Option<String>,
    /// R25 — the item's PreservePreviousForm flag.
    pub preserve_previous_form: bool,
}

// ── Navigation chain (R19–R27) ──────────────────────────────────────────────

/// A resident form's machinery, owned by its [`NavEntry`]. The glue implements
/// this over the real interpreter thread + channels; the shell only speaks
/// lifecycle. Being in the chain (or parked) IS residency (R20): storage lives
/// exactly as long as its `Resident` box.
pub trait Resident {
    /// R26 — `onDeactivate`: the body left the ContentPane; the form stays
    /// resident. Never a teardown point (R27).
    fn deactivate(&mut self);
    /// R26 — `onDestroy`: fired immediately before the box is dropped and the
    /// form's storage is released.
    fn destroy(&mut self);
}

/// The standard [`Resident`]: lifecycle calls become [`FormEvent`]s on the
/// form's own event channel, so the generated program's dispatch loop runs
/// the developer's `onDeactivate`/`onDestroy` handlers (R26). Glue that owns
/// an interpreter thread wraps this and adds its teardown after `destroy`.
pub struct ChannelResident {
    pub form_object: String,
    pub ev_tx: std::sync::mpsc::Sender<cobolt_runtime::channels::FormEvent>,
}

impl Resident for ChannelResident {
    fn deactivate(&mut self) {
        let _ = self.ev_tx.send(cobolt_runtime::channels::FormEvent::new(
            self.form_object.clone(),
            "onDeactivate",
        ));
    }
    fn destroy(&mut self) {
        let _ = self.ev_tx.send(cobolt_runtime::channels::FormEvent::new(
            self.form_object.clone(),
            "onDestroy",
        ));
    }
}

/// One link of the navigation chain: main form → … → the displayed form.
pub struct NavEntry {
    /// UPPERCASE form object name.
    pub form_object: String,
    /// The breadcrumb segment label (R21).
    pub label: String,
    /// Loads carrying `PreservePreviousForm` (R24/R25) park this entry
    /// instead of destroying it when a SIBLING replaces it.
    pub preserve_on_replace: bool,
    pub resident: Box<dyn Resident>,
}

/// R19 — the ordered chain of resident forms. Entry 0 is the main form; the
/// LAST entry is the displayed one. Every entry is resident (R20) — ancestors
/// are alive because something below them is their child; `parked` holds
/// preserved siblings (R25) awaiting instant return.
#[derive(Default)]
pub struct NavChain {
    entries: Vec<NavEntry>,
    parked: Vec<NavEntry>,
}

impl NavChain {
    /// The breadcrumb's segments, chain order (R21).
    pub fn segments(&self) -> Vec<(String, String)> {
        self.entries
            .iter()
            .map(|e| (e.form_object.clone(), e.label.clone()))
            .collect()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The currently displayed form (the last entry).
    pub fn current(&self) -> Option<&NavEntry> {
        self.entries.last()
    }

    /// How many forms are resident in total (chain + parked) — R20's
    /// observable.
    pub fn resident_count(&self) -> usize {
        self.entries.len() + self.parked.len()
    }

    /// Push the application's first entry (the main form) or a CHILD of the
    /// current form: the current top deactivates (stays resident — it is now
    /// an ancestor) and the new entry is displayed.
    pub fn push(&mut self, entry: NavEntry) {
        if let Some(top) = self.entries.last_mut() {
            top.resident.deactivate();
        }
        self.entries.push(entry);
    }

    /// R25 — a SIBLING load: the displayed form is replaced under the same
    /// parent. The outgoing top is parked when its load asked for
    /// preservation, destroyed otherwise. A parked twin of the incoming form
    /// is revived instead of `incoming` when present (instant return).
    /// Returns true when the displayed entry came from the parking lot.
    pub fn replace_top(&mut self, incoming: NavEntry) -> bool {
        if let Some(mut top) = self.entries.pop() {
            if top.preserve_on_replace {
                top.resident.deactivate();
                self.parked.push(top);
            } else {
                top.resident.destroy();
            }
        }
        if let Some(at) = self
            .parked
            .iter()
            .position(|p| p.form_object == incoming.form_object)
        {
            // Instant return: the parked storage IS the point (R24 tip).
            let revived = self.parked.remove(at);
            drop(incoming);
            self.entries.push(revived);
            return true;
        }
        self.entries.push(incoming);
        false
    }

    /// Push the entry that REPLACES the one a reset just destroyed.
    ///
    /// Unlike [`Self::push`] the parent is not deactivated: it never stopped
    /// being an ancestor, and firing `onDeactivate` at it a second time would
    /// report a transition that never happened.
    pub fn push_restarted(&mut self, entry: NavEntry) {
        self.entries.push(entry);
    }

    /// R22 — truncate to `index` (a breadcrumb click): every entry BELOW it
    /// is destroyed deepest-first; the target becomes the displayed form.
    /// Returns the destroyed form objects, in destruction order.
    pub fn pop_to(&mut self, index: usize) -> Vec<String> {
        let mut destroyed = Vec::new();
        while self.entries.len() > index + 1 {
            let mut e = self.entries.pop().expect("len checked");
            e.resident.destroy();
            destroyed.push(e.form_object);
        }
        destroyed
    }

    /// 051 — the incoming load's `PreservePreviousForm` decides the fate of
    /// the form DISPLAYED BEFORE it (049 R24): the shell marks the top with
    /// the clicking item's flag just before [`Self::replace_top`] judges it.
    pub fn mark_top_preserve(&mut self, preserve: bool) {
        if let Some(top) = self.entries.last_mut() {
            top.preserve_on_replace = preserve;
        }
    }

    /// **Home** — show the MAIN form again without destroying anything.
    ///
    /// Every entry above the root is PARKED (deactivated, still resident)
    /// rather than destroyed, which is the whole difference from a breadcrumb
    /// click on segment 0: the shell's own content pane comes back, no
    /// `onDestroy` fires, no occupant is retired, and every other live form —
    /// pane occupant or child window — carries on untouched. A later load of
    /// a parked form revives that same instance, with its WORKING-STORAGE
    /// exactly as it left it ([`Self::push_parked`]).
    ///
    /// A no-op at the root, so Home twice does not deactivate the main form.
    pub fn park_to_root(&mut self) {
        if self.entries.len() <= 1 {
            return;
        }
        // Only the DISPLAYED form is active, so only it deactivates. Its
        // ancestors already did when their child was pushed, and firing
        // `onDeactivate` at them a second time would report a transition that
        // never happened — a handler counting activations would drift.
        if let Some(top) = self.entries.last_mut() {
            top.resident.deactivate();
        }
        while self.entries.len() > 1 {
            let e = self.entries.pop().expect("len checked");
            self.parked.push(e);
        }
    }

    /// Display a PARKED form again as a child of the current top — the shape
    /// a load takes once Home has parked it. Without this, loading a form
    /// after Home would push a SECOND entry for a form already sitting in the
    /// parking lot: two chain entries and one live occupant, with the parked
    /// one unreachable and never destroyed. `false` = nothing parked under
    /// that name, so the caller pushes a fresh entry.
    pub fn push_parked(&mut self, form_object: &str) -> bool {
        let Some(at) = self
            .parked
            .iter()
            .position(|p| p.form_object == form_object)
        else {
            return false;
        };
        let revived = self.parked.remove(at);
        if let Some(top) = self.entries.last_mut() {
            top.resident.deactivate();
        }
        self.entries.push(revived);
        true
    }

    /// R23 — a root-slot subsystem switch: unwind to the MAIN form (also
    /// destroying the parking lot — a preserved sibling of a dead subsystem
    /// has no way back), then the caller pushes the new subsystem.
    pub fn unwind_to_root(&mut self) -> Vec<String> {
        let mut destroyed = self.pop_to(0);
        for mut p in self.parked.drain(..) {
            p.resident.destroy();
            destroyed.push(p.form_object);
        }
        destroyed
    }
}

// ── Navigation operations (R22/R23/R25) ─────────────────────────────────────
//
// The glue supplies `menu_of`: the sidecar menu of a form object, if it has
// one (`menu_yaml_path` next to its `.cfrm`).

/// R22 — a breadcrumb click: truncate the chain at `index` (onDestroy fires
/// deepest-first inside `pop_to`), make that form current, and remount its
/// menu into the contextual slot — the ROOT slot never moves. Index 0 (the
/// main form) clears the contextual slot. Returns the destroyed form objects
/// in destruction order.
pub fn breadcrumb_pop(
    shell: &mut Shell,
    chain: &mut NavChain,
    index: usize,
    menu_of: &dyn Fn(&str) -> Option<cobolt_forms::menu::MenuDefinition>,
) -> Vec<String> {
    let destroyed = chain.pop_to(index);
    if index == 0 {
        shell.mount_contextual_menu(None);
    } else if let Some(current) = chain.current().map(|e| e.form_object.clone()) {
        shell.mount_contextual_menu(menu_of(&current).map(|d| (current, d)));
    }
    destroyed
}

/// R23 — a ROOT-slot subsystem switch: unwind to the main form first (every
/// form below it, and the parking lot, receives onDestroy), then push the new
/// subsystem and mount its menu. Returns the destroyed form objects.
pub fn root_switch(
    shell: &mut Shell,
    chain: &mut NavChain,
    subsystem: NavEntry,
    menu: Option<cobolt_forms::menu::MenuDefinition>,
) -> Vec<String> {
    let destroyed = chain.unwind_to_root();
    let form = subsystem.form_object.clone();
    chain.push(subsystem);
    shell.mount_contextual_menu(menu.map(|d| (form, d)));
    destroyed
}

/// What a click on the breadcrumb strip landed on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BreadcrumbHit {
    /// The sidebar's Open/Collapsed control, at the head of the strip.
    pub toggle: bool,
    /// A chain segment, which the navigation layer feeds to
    /// [`NavChain::pop_to`] (R22).
    pub segment: Option<usize>,
}

/// The shell chrome: pane widths and the Open/Collapsed state (R8). The
/// persistence of `collapsed` (R9) and the mounted menus (R6/R7) arrive with
/// their own tasks — this type owns layout.
pub struct Shell {
    pub menu_open_width: f32,
    pub menu_collapsed_width: f32,
    pub collapsed: bool,
    /// 049 — the SideMenu's `FullHeight` property (default on). On, the
    /// MenuPane owns the window's whole vertical extent and the breadcrumb
    /// starts at its right edge; off, the breadcrumb spans the full width and
    /// the MenuPane fills the height beneath it. Either way the pane fills its
    /// column top to bottom, in both Open and Collapsed states.
    pub full_height: bool,
    /// Set when the pane's own toggle was clicked this frame; drained by the
    /// owner (which also persists the new state) with
    /// [`Self::take_toggle_request`].
    toggle_requested: bool,
    /// R6 — the root slot: the main form's menu, mounted once.
    root_menu: Option<MountedMenu>,
    /// R6/R7 — the contextual slot: the current subsystem's menu.
    contextual_menu: Option<MountedMenu>,
    /// Clicks collected this frame, drained with [`Self::take_menu_clicks`].
    pending_clicks: Vec<MenuClick>,
    /// R39 — the MenuPane's OWN background (from the main form's
    /// `MenuPaneBackground` group). `None` = the default chrome fill. A form
    /// loaded into the ContentPane can never repaint this: it is painted by
    /// the shell, in the menu panel, from shell state.
    pub menu_background: Option<cobolt_forms::model::MenuPaneBackground>,
    /// The SideMenu's `IconEffect` property (`None` | `Shadow` | `Neumorphic`)
    /// — how menu-item icons are painted in the pane.
    pub icon_effect: String,
    /// A clone of the designed SideMenu control, so the pane takes its
    /// colours, title and profile card from the application's own theme
    /// rather than from constants baked into the shell.
    pub side_ctrl: Option<cobolt_forms::Control>,
    /// The MAIN form's own resolved backdrop colour. The rail paints ON this,
    /// never straight onto the window: a SideMenu's `BackgroundColor` is
    /// commonly translucent, and on the designer canvas that translucency
    /// composites over the form. Painting the bare colour into a transparent
    /// shell window (R43) instead composited it over the DESKTOP, which is why
    /// a rail designed navy shipped white on a light-mode desktop.
    pub form_backdrop: Option<egui::Color32>,
    /// R21 — the chain the strip renders, root first. The owner refreshes it
    /// from its [`NavChain`] each frame; the shell paints it.
    pub breadcrumb: Vec<String>,
    /// The DETAIL level the displayed form appended after its own name
    /// (`me::"SetBreadcrumbDetail"`). The owner refreshes it — and drops it on
    /// every navigation — so a crumb can never outlive the form that set it
    /// and end up hanging off somebody else's name.
    pub detail: Option<String>,
    /// The breadcrumb frame's height, from the rail's `BreadcrumbHeight`.
    pub breadcrumb_height: f32,
    /// The frame's own background, from the rail's `BreadcrumbBackgroundColor`.
    /// `None` = follow the ContentPane's backdrop, as it always has.
    pub breadcrumb_bg: Option<egui::Color32>,
    /// Parent item ids whose children are expanded in place.
    pub expanded: Vec<String>,
    /// How far the MenuPane's rows are scrolled (see `draw_mounted_menus`).
    menu_scroll: f32,
    /// A breadcrumb segment clicked this frame, drained with
    /// [`Self::take_breadcrumb_click`].
    pending_crumb: Option<usize>,
    /// The displayed form's OWN segment was clicked while a detail level sat
    /// after it — a request to start that form over. Drained with
    /// [`Self::take_reset_request`].
    pending_reset: bool,
    /// What the strip laid out last frame (tests click the segment they mean
    /// instead of a coordinate that moves whenever the chrome changes).
    last_crumb_layout: Option<cobolt_forms::breadcrumb::BreadcrumbLayout>,
    /// What the MenuPane actually painted last frame (tests/parity).
    last_menu_fill: Option<egui::Color32>,
    /// Where the Open/Collapsed toggle landed last frame (tests/parity).
    last_toggle_rect: Option<Rect>,
    /// Where the rail's FOOTER BAND landed last frame — the band the SideMenu's
    /// footer Panel and its controls are drawn into. Recorded while the rail is
    /// laid out, spent once the pane's `Ui` is in hand.
    last_footer_rect: Option<Rect>,
    /// Where each drawn menu item landed last frame, by item id. Lets a test
    /// click the item it means instead of a magic coordinate that shifts
    /// whenever the pane gains chrome.
    last_item_rects: Vec<(String, Rect)>,
}

impl Default for Shell {
    fn default() -> Self {
        Self {
            menu_open_width: MENU_PANE_OPEN_WIDTH,
            menu_collapsed_width: MENU_PANE_COLLAPSED_WIDTH,
            collapsed: false,
            full_height: true,
            toggle_requested: false,
            root_menu: None,
            contextual_menu: None,
            pending_clicks: Vec::new(),
            menu_background: None,
            icon_effect: "None".to_owned(),
            side_ctrl: None,
            form_backdrop: None,
            breadcrumb: Vec::new(),
            detail: None,
            breadcrumb_height: BREADCRUMB_HEIGHT,
            breadcrumb_bg: None,
            expanded: Vec::new(),
            menu_scroll: 0.0,
            pending_crumb: None,
            pending_reset: false,
            last_crumb_layout: None,
            last_menu_fill: None,
            last_toggle_rect: None,
            last_footer_rect: None,
            last_item_rects: Vec::new(),
        }
    }
}

impl Shell {
    /// The MenuPane's current width — fixed per state, never derived from the
    /// window or content size (R38).
    pub fn menu_pane_width(&self) -> f32 {
        if self.collapsed {
            self.menu_collapsed_width
        } else {
            self.menu_open_width
        }
    }

    /// R39 — paint the MenuPane's own background into its panel `Ui`,
    /// through the SAME `paint_backdrop` every form background uses (one
    /// background dialect). Records the resolved fill for the parity suite.
    fn paint_menu_background(&mut self, ui: &Ui) {
        // R43 — the base the rail composites onto, and it is ALWAYS opaque:
        // the form's own backdrop over the chrome constant, so a form the
        // developer made translucent still cannot punch a hole in the chrome.
        let base = over(
            self.form_backdrop.unwrap_or(egui::Color32::TRANSPARENT),
            CHROME_FILL,
        );
        ui.painter().rect_filled(ui.max_rect(), 0.0, base);

        let Some(mp) = &self.menu_background else {
            // With no MenuPaneBackground group configured, the rail's own
            // `BackgroundColor` is the colour — and the SHARED renderer paints
            // it, over this base, exactly as it does on the other three
            // surfaces. `draw_mounted_menus` records the resolved fill.
            self.last_menu_fill = Some(base);
            return;
        };
        let backdrop = cobolt_forms::render::Backdrop {
            paint: true,
            color_hex: mp.color.clone(),
            transparency: mp.transparency,
            gradient_enabled: mp.gradient_enabled,
            gradient_start_hex: mp.gradient_start_color.clone(),
            gradient_end_hex: mp.gradient_end_color.clone(),
            gradient_direction: mp.gradient_direction.clone(),
            // The image is resolved to a texture by the hosting glue; the
            // shell paints colour + gradient. (Same contract as the engine:
            // "the engine has no texture cache".)
            image: None,
            image_mode: mp.image_mode,
            use_theme_background: false,
            window_size: None,
        };
        let painted =
            cobolt_forms::render::paint_backdrop(ui.painter(), ui.max_rect(), &backdrop);
        // The group's colour lands on the same opaque base, so a transparency
        // set on it shows the application through — not the desktop.
        self.last_menu_fill = Some(over(painted.bg, base));
    }

    /// What the MenuPane painted last frame (`None` = default chrome).
    pub fn menu_fill(&self) -> Option<egui::Color32> {
        self.last_menu_fill
    }

    /// R6 — mount the MAIN form's menu into the root slot. First mount wins:
    /// the root slot is never replaced for the life of the application.
    pub fn mount_root_menu(&mut self, form_object: &str, def: cobolt_forms::menu::MenuDefinition) {
        if self.root_menu.is_none() {
            self.root_menu = Some(MountedMenu {
                form_object: form_object.trim().to_ascii_uppercase(),
                def,
            });
        }
    }

    /// R7 — replace the contextual slot WHOLESALE with the current
    /// subsystem's menu (`None` clears it, e.g. back at the main form).
    pub fn mount_contextual_menu(
        &mut self,
        menu: Option<(String, cobolt_forms::menu::MenuDefinition)>,
    ) {
        self.contextual_menu = menu.map(|(form_object, def)| MountedMenu {
            form_object: form_object.trim().to_ascii_uppercase(),
            def,
        });
    }

    /// The mounted slots (root, contextual) — inspection for tests/nav.
    pub fn mounted(&self) -> (Option<&MountedMenu>, Option<&MountedMenu>) {
        (self.root_menu.as_ref(), self.contextual_menu.as_ref())
    }

    /// Drain the clicks the MenuPane collected this frame.
    pub fn take_menu_clicks(&mut self) -> Vec<MenuClick> {
        std::mem::take(&mut self.pending_clicks)
    }

    /// The breadcrumb strip — the same chrome in both layout orders, so
    /// FullHeight changes only WHEN it is created, never what it is.
    ///
    /// Painted by the SHARED renderer (`cobolt_forms::breadcrumb`), which is
    /// the very code the designer canvas and the preview draw: the strip the
    /// developer designs against and the strip their operator gets are one
    /// drawing. The strip also carries the sidebar's Open/Collapsed control at
    /// its head — a full-height icon cell, in the rail's own colours.
    /// The strip's background: the developer's own `BreadcrumbBackgroundColor`
    /// when they set one, otherwise the ContentPane's backdrop. Opaque either
    /// way (R43) — the strip is chrome, and a hole in chrome shows the desktop.
    fn crumb_bg(&self) -> egui::Color32 {
        let pane = over(
            self.form_backdrop.unwrap_or(egui::Color32::TRANSPARENT),
            CHROME_FILL,
        );
        match self.breadcrumb_bg {
            Some(c) => over(c, pane),
            None => pane,
        }
    }

    /// Paint the strip and hit-test what it drew, on `rect`. Shared by the
    /// panel path ([`Self::show_breadcrumb`]) and the OVERLAY path the shell
    /// host uses, so both surfaces are one drawing.
    fn paint_crumb(&mut self, ctx: &egui::Context, painter: &egui::Painter, rect: Rect) {
        use cobolt_forms::breadcrumb as bc;
        let bg = self.crumb_bg();
        let segments = self.breadcrumb.clone();
        let mut state = match &self.side_ctrl {
            Some(c) => bc::state_for_control(ctx, c, &segments, bg),
            None => bc::state_plain(&segments, bg),
        };
        state.detail = self.detail.clone();
        // The rail's LIVE state, not the designed one: the arrow has to show
        // what the next click does.
        state.collapsed = self.collapsed;
        let layout = bc::layout(painter, rect, &state);
        state.toggle_hovered = ctx
            .pointer_interact_pos()
            .is_some_and(|p| bc::toggle_hit(&layout, p));
        bc::paint(painter, rect, &state, &layout);
        self.last_crumb_layout = Some(layout);
    }

    /// Register the strip's clicks against what it laid out LAST frame.
    ///
    /// Registered BEFORE the form renders, so a control the developer placed
    /// over the frame wins the pointer: the band is chrome, and chrome never
    /// steals a click from the developer's own control.
    fn crumb_interact(&mut self, ui: &mut Ui) {
        let Some(layout) = self.last_crumb_layout.clone() else {
            return;
        };
        if ui
            .interact(
                layout.toggle,
                ui.id().with("crumb-toggle"),
                egui::Sense::click(),
            )
            .clicked()
        {
            self.toggle_requested = true;
        }
        let reset_at = layout.reset_segment();
        for (i, seg) in layout.segments.iter().enumerate() {
            if ui
                .interact(*seg, ui.id().with(("crumb-seg", i)), egui::Sense::click())
                .clicked()
            {
                // The displayed form's own name, with a detail level after it,
                // is a RESET — everything above it is a navigation.
                if reset_at == Some(i) {
                    self.pending_reset = true;
                } else {
                    self.pending_crumb = Some(i);
                }
            }
        }
    }

    /// Lay the strip out now and hand back a painter for it, to be run between
    /// the ContentPane's backdrop and the form's controls.
    ///
    /// The layout is computed HERE, with the pane's own painter, and travels
    /// into the closure: the strip is hit-tested against the very rects it
    /// paints, and the shell keeps a copy so the next frame's clicks land on
    /// what the operator is actually looking at.
    fn crumb_chrome(
        &mut self,
        ui: &Ui,
        rect: Rect,
    ) -> Box<dyn Fn(&egui::Painter, egui::Rect)> {
        use cobolt_forms::breadcrumb as bc;
        let ctx = ui.ctx().clone();
        let bg = self.crumb_bg();
        let segments = self.breadcrumb.clone();
        let detail = self.detail.clone();
        let side = self.side_ctrl.clone();
        let collapsed = self.collapsed;
        let (layout, hovered) = {
            let mut state = match &side {
                Some(c) => bc::state_for_control(&ctx, c, &segments, bg),
                None => bc::state_plain(&segments, bg),
            };
            state.detail = detail.clone();
            state.collapsed = collapsed;
            let layout = bc::layout(ui.painter(), rect, &state);
            let hovered = ctx
                .pointer_interact_pos()
                .is_some_and(|p| bc::toggle_hit(&layout, p));
            (layout, hovered)
        };
        self.last_crumb_layout = Some(layout.clone());
        Box::new(move |painter: &egui::Painter, _pane_rect: egui::Rect| {
            let mut state = match &side {
                Some(c) => bc::state_for_control(&ctx, c, &segments, bg),
                None => bc::state_plain(&segments, bg),
            };
            state.detail = detail.clone();
            state.collapsed = collapsed;
            state.toggle_hovered = hovered;
            bc::paint(painter, rect, &state, &layout);
        })
    }

    fn show_breadcrumb(&mut self, root_ui: &mut Ui) -> Rect {
        let height = self.breadcrumb_height;
        let panel = egui::Panel::top("shell-breadcrumb")
            .resizable(false)
            .exact_size(height)
            // R43 — the chrome supplies its OWN frame. egui's default panel
            // frame paints `visuals.panel_fill` — which follows the OS
            // light/dark theme — and then insets the closure's `max_rect` by
            // its margin, so the strip shipped with a thick pale border around
            // it on a light-mode desktop: the frame's fill showing through the
            // margin our explicit paint could not reach. The shared renderer
            // paints the strip edge to edge, so there is no frame at all.
            .frame(egui::Frame::NONE)
            // No separator line either — see the MenuPane.
            .show_separator_line(false)
            .show(root_ui, |ui| {
                let rect = ui.max_rect();
                let ctx = ui.ctx().clone();
                let painter = ui.painter().clone();
                self.paint_crumb(&ctx, &painter, rect);
                // One interaction per laid-out rect — never a re-derived one.
                self.crumb_interact(ui);
            });
        panel.response.rect
    }

    /// Drain a breadcrumb segment click (R22).
    pub fn take_breadcrumb_click(&mut self) -> Option<usize> {
        self.pending_crumb.take()
    }

    /// Drain a RESET request — the displayed form's own segment clicked while
    /// a detail level sat after it.
    pub fn take_reset_request(&mut self) -> bool {
        std::mem::take(&mut self.pending_reset)
    }

    /// What the strip laid out last frame.
    pub fn crumb_layout(&self) -> Option<&cobolt_forms::breadcrumb::BreadcrumbLayout> {
        self.last_crumb_layout.as_ref()
    }

    /// Drain a click on the pane's Open/Collapsed toggle. The caller flips
    /// [`Self::collapsed`] and persists it (R9) — the shell does not persist
    /// on its own, so tests can drive the toggle without touching disk.
    pub fn take_toggle_request(&mut self) -> bool {
        std::mem::take(&mut self.toggle_requested)
    }

    /// Where the pane's toggle landed last frame (tests drive it from here).
    /// The toggle is the rail's header row — see `draw_mounted_menus`.
    pub fn toggle_rect(&self) -> Option<Rect> {
        self.last_toggle_rect
    }

    /// Draw both mounted slots into the MenuPane's scroll `Ui` and collect
    /// clicks. Open: labels (submenu items indented). Collapsed: the rail
    /// keeps the ROOT items reachable as single-glyph buttons (R8).
    fn draw_mounted_menus(&mut self, ui: &mut Ui) {
        use cobolt_forms::menu::MenuItem;
        use cobolt_forms::sidebar::{self, RowKind};

        // Both mounted slots become ONE item list, so the shared renderer sees
        // the rail the way the designer canvas and the preview do. The slot a
        // click belongs to is recovered from its index: root items come first,
        // then a divider, then the contextual slot.
        let root = self.root_menu.clone();
        let ctx_menu = self.contextual_menu.clone();
        let mut items: Vec<MenuItem> = Vec::new();
        if let Some(r) = &root {
            items.extend(r.def.menu.iter().cloned());
        }
        let root_len = items.len();
        let mut divider = 0usize;
        if let Some(c) = &ctx_menu {
            if root_len > 0 {
                items.push(MenuItem::new_separator("__slot-divider__"));
                divider = 1;
            }
            items.extend(c.def.menu.iter().cloned());
        }

        let rect = ui.max_rect();
        let ctrl = self.side_control();
        let expanded = self.expanded.clone();
        let mut state = sidebar::state_for_control(ui.ctx(), &ctrl, &items, 255, &expanded);

        // The rail's designed background, over the opaque base the chrome
        // painted. R39's `MenuPaneBackground` group, when the developer
        // configured one, IS the pane's background and has already been
        // painted — the rail must not tint it, so it contributes nothing.
        state.backdrop = over(
            self.form_backdrop.unwrap_or(egui::Color32::TRANSPARENT),
            CHROME_FILL,
        );
        if self.menu_background.is_some() {
            state.bg = egui::Color32::TRANSPARENT;
        } else {
            self.last_menu_fill = Some(cobolt_forms::paint::composite_premultiplied_over(
                state.bg,
                state.backdrop,
            ));
        }

        // The menu pane scrolls when the mounted slots are taller than it is —
        // two slots concatenated overflow easily. The header and footer panes
        // do not move, so the toggle stays reachable at any scroll.
        state.scroll = self.menu_scroll;
        let max_scroll = sidebar::max_scroll(rect, &state);
        let pointer = ui.ctx().pointer_interact_pos().filter(|p| rect.contains(*p));
        if max_scroll > 0.0 && pointer.is_some() {
            let dy = ui.input(|i| i.smooth_scroll_delta.y);
            if dy != 0.0 {
                state.scroll -= dy;
            }
        }
        state.scroll = state.scroll.clamp(0.0, max_scroll);
        self.menu_scroll = state.scroll;

        let rows = sidebar::layout(rect, &state);
        state.hovered = pointer.and_then(|p| sidebar::row_at(&rows, p));
        sidebar::paint(ui.painter(), rect, &rows, &state);
        // Where the footer Panel and its controls go. Taken from the SAME
        // layout the rail was painted from, so the panel cannot land anywhere
        // but on the band under it — through a form resize, a `FooterHeight`
        // edit or a collapse.
        self.last_footer_rect = rows
            .iter()
            .find(|r| matches!(r.kind, RowKind::Footer))
            .map(|r| r.rect);

        let mut clicks = Vec::new();
        let mut rects: Vec<(String, Rect)> = Vec::new();
        let mut flip: Option<String> = None;
        // The designed SideMenu's `Cursor` covers its ROWS — the rail itself is
        // a pane you never point at — exactly as the engine's sidebar arm does.
        let row_cursor = self
            .side_ctrl
            .as_ref()
            .and_then(|c| c.get_prop("Cursor"))
            .and_then(|v| cobolt_forms::render::cursor_icon_for(v.as_str()));
        for (ix, row) in rows.iter().enumerate() {
            // The row's VISIBLE part — see the same rule in the engine's
            // sidebar arm: a scrolled row must not take clicks where it is
            // hidden under the header or footer pane.
            let mut resp = ui.interact(
                row.visible,
                ui.id().with(("shell-side-row", ix)),
                egui::Sense::click(),
            );
            if let Some(icon) = row_cursor {
                resp = resp.on_hover_cursor(icon);
            }
            match &row.kind {
                RowKind::Header => {
                    // The header IS the toggle — the rail stays collapsible
                    // with an empty menu, which is the standing requirement.
                    self.last_toggle_rect = Some(row.rect);
                    if resp.clicked() {
                        self.toggle_requested = true;
                    }
                }
                RowKind::Item { id, path, .. } => {
                    rects.push((id.clone(), row.rect));
                    let Some(item) = sidebar::item_at(&items, path) else {
                        continue;
                    };
                    if !resp.clicked() || !item.enabled {
                        continue;
                    }
                    if item.has_children() && !self.collapsed {
                        flip = Some(id.clone());
                        continue;
                    }
                    let slot = if path[0] < root_len {
                        MenuSlot::Root
                    } else {
                        MenuSlot::Contextual
                    };
                    clicks.push(MenuClick {
                        slot,
                        item_id: item.id.clone(),
                        action: item.action.clone(),
                        preserve_previous_form: item.preserve_previous_form,
                    });
                }
                _ => {}
            }
        }
        let _ = divider;
        if let Some(pid) = flip {
            if let Some(p) = self.expanded.iter().position(|e| e == &pid) {
                self.expanded.remove(p);
            } else {
                self.expanded.push(pid);
            }
        }
        self.pending_clicks.extend(clicks);
        self.last_item_rects = rects;
    }

    /// The SideMenu control the rail takes its colours and chrome from. The
    /// shell keeps a clone of it; tests that drive a bare `Shell` get a
    /// default-styled control instead of having to build a form.
    fn side_control(&self) -> cobolt_forms::Control {
        let mut ctrl = self.side_ctrl.clone().unwrap_or_else(|| {
            cobolt_forms::Control::new(
                "SIDE",
                cobolt_forms::ControlType::SideMenu,
                0,
                0,
            )
        });
        // The pane's own state wins over whatever the designed control said.
        ctrl.set_prop("Collapsed", self.collapsed);
        ctrl.set_prop("IconEffect", self.icon_effect.clone());
        ctrl
    }

    /// Where a menu item was drawn last frame, by item id.
    pub fn item_rect(&self, item_id: &str) -> Option<Rect> {
        self.last_item_rects
            .iter()
            .find(|(id, _)| id == item_id)
            .map(|(_, r)| *r)
    }

    /// Lay out the three regions on `root_ui` (the window's root `Ui`, the
    /// same surface [`crate::FormHost::ui`] renders on) and run the given
    /// closures inside them. The menu and content closures get a `Ui` INSIDE
    /// that pane's own scroll area; the breadcrumb strip does not scroll —
    /// it is a fixed line of chrome.
    pub fn show(
        &mut self,
        root_ui: &mut Ui,
        menu: impl FnOnce(&mut Ui),
        content: impl FnOnce(&mut Ui),
    ) -> ShellLayout {
        let mut menu_scroll = Vec2::ZERO;
        // 049 — panel ORDER is what decides who owns the corner: whichever is
        // created first spans its full axis. FullHeight therefore reads as
        // "the MenuPane goes in first".
        let mut breadcrumb_rect = Rect::NOTHING;
        let mut crumb_done = false;
        if !self.full_height {
            breadcrumb_rect = self.show_breadcrumb(root_ui);
            crumb_done = true;
        }
        let panel = egui::Panel::left("shell-menu-pane")
            .resizable(false)
            .exact_size(self.menu_pane_width())
            // No default frame: its OS-theme fill and its margin would both
            // sit outside the rail's own paint (see `show_breadcrumb`). The
            // rail is painted edge to edge by `paint_menu_background`.
            .frame(egui::Frame::NONE)
            // …and no separator line. egui draws one on a panel's inner edge;
            // the shell's regions are told apart by their own colour, and the
            // rule just read as an unwanted border down the rail.
            .show_separator_line(false)
            .show(root_ui, |ui| {
                // R39 — the shell's own chrome paint, before any content.
                self.paint_menu_background(ui);
                // R37 — the MenuPane's own scroll area, id distinct from the
                // ContentPane's by construction.
                let out = egui::ScrollArea::vertical()
                    .id_salt("shell-menu-scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        // R6/R7 — the mounted slots draw next; the caller's
                        // closure may add extra chrome below them.
                        self.draw_mounted_menus(ui);
                        menu(ui)
                    });
                menu_scroll = out.state.offset;
            });
        let menu_rect = panel.response.rect;

        if !crumb_done {
            breadcrumb_rect = self.show_breadcrumb(root_ui);
        }

        let mut content_rect = Rect::NOTHING;
        let mut content_scroll = Vec2::ZERO;
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(root_ui, |ui| {
                content_rect = ui.max_rect();
                // R40 — the ContentPane's own scroll area: a form larger than
                // the pane scrolls inside it (both axes — the form keeps its
                // designed pixel size, R11).
                let out = egui::ScrollArea::both()
                    .id_salt("shell-content-scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| content(ui));
                content_scroll = out.state.offset;
            });

        ShellLayout {
            menu_rect,
            breadcrumb_rect,
            content_rect,
            menu_scroll,
            content_scroll,
        }
    }

    /// Lay out the shell with a [`crate::FormHost`] (in `Pane` mode) as the
    /// ContentPane's occupant (R10/R11). The host renders through its OWN
    /// `CentralPanel` + `ScrollArea` on the pane's `Ui` — the shell must NOT
    /// wrap it in a second scroll area (nested both-axis scroll areas fight
    /// over the wheel), so the host's scroll IS the ContentPane scroll (R40).
    /// The form anchors at the pane's top-left and travels with the pane edge
    /// when the MenuPane changes state, keeping its designed size.
    pub fn show_with_host(
        &mut self,
        root_ui: &mut Ui,
        menu: impl FnOnce(&mut Ui),
        host: &mut crate::FormHost,
    ) -> ShellLayout {
        let mut menu_scroll = Vec2::ZERO;
        // See `show` — panel order is what makes FullHeight true or false.
        let mut breadcrumb_rect = Rect::NOTHING;
        let mut crumb_done = false;
        if !self.full_height {
            breadcrumb_rect = self.show_breadcrumb(root_ui);
            crumb_done = true;
        }
        let panel = egui::Panel::left("shell-menu-pane")
            .resizable(false)
            .exact_size(self.menu_pane_width())
            // No default frame: its OS-theme fill and its margin would both
            // sit outside the rail's own paint (see `show_breadcrumb`). The
            // rail is painted edge to edge by `paint_menu_background`.
            .frame(egui::Frame::NONE)
            // …and no separator line. egui draws one on a panel's inner edge;
            // the shell's regions are told apart by their own colour, and the
            // rule just read as an unwanted border down the rail.
            .show_separator_line(false)
            .show(root_ui, |ui| {
                // R39 — the shell's own chrome paint, before any content.
                self.paint_menu_background(ui);
                let out = egui::ScrollArea::vertical()
                    .id_salt("shell-menu-scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        // R6/R7 — the mounted slots draw next.
                        self.draw_mounted_menus(ui);
                        menu(ui)
                    });
                menu_scroll = out.state.offset;
                // The developer's footer Panel, on the band the rail just laid
                // out. Drawn OUTSIDE the scroll area, like the band itself: the
                // footer does not move when the menu scrolls, so neither may
                // what sits in it.
                if let Some(band) = self.last_footer_rect {
                    // What the rail actually painted under the band, so a
                    // Panel the developer left translucent resolves against the
                    // rail — and so nothing paints a background over it.
                    let behind = self.last_menu_fill.unwrap_or(CHROME_FILL);
                    host.draw_side_menu_footer(ui, band, behind);
                }
            });
        let menu_rect = panel.response.rect;

        // The remaining space IS the ContentPane; the host's own
        // CentralPanel + ScrollArea consume it.
        let content_rect = root_ui.available_rect_before_wrap();
        if !crumb_done {
            // FullHeight ON — the frame is the top BAND of the content area,
            // drawn as an OVERLAY rather than as a panel above it: the form's
            // own coordinate space starts at the top of the band, exactly as
            // the designer canvas draws it, so a control the developer placed
            // over the frame paints on top of it. The frame is chrome, not a
            // container: that control is nobody's child and is clipped by
            // nothing.
            breadcrumb_rect = Rect::from_min_size(
                content_rect.min,
                Vec2::new(content_rect.width(), self.breadcrumb_height),
            );
            // Registered BEFORE the form renders, so the developer's own
            // control over the band wins the pointer.
            self.crumb_interact(root_ui);
            let chrome = self.crumb_chrome(root_ui, breadcrumb_rect);
            host.set_pane_chrome(Some(chrome), self.breadcrumb_height);
        }
        host.pane_frame(root_ui);

        ShellLayout {
            menu_rect,
            breadcrumb_rect,
            content_rect,
            menu_scroll,
            content_scroll: Vec2::ZERO,
        }
    }
}

// ── The shell host entry (R2/R3, the run-form glue's shell branch) ──────────

/// Build the ONE shell window and run it to completion (the shell-mode
/// sibling of [`crate::run`]). The MAIN form is hosted in the ContentPane
/// (`Surface::Pane` is forced); `root_menu` is its SideMenu's sidecar
/// definition, mounted into the root slot (R6). The window is created
/// TRANSPARENT (R43): the chrome paints itself opaque, and only a
/// transparent form's pane region reaches the desktop.
///
/// Menu items that load OTHER forms (`open-form:`) are reported, not yet
/// performed: hosting a second form's interpreter is the same open work as
/// 037 T16's child windows — see the 049 tasks note.
pub fn run_shell(
    mut config: crate::host::FormHostConfig,
    root_menu: Option<(String, cobolt_forms::menu::MenuDefinition)>,
) {
    config.surface = crate::Surface::Pane;
    let ev_tx = config.ev_tx.clone();
    let input_tx = config.input_tx.clone();
    let form_req_tx = config.form_req_tx.clone();
    let title_fallback = config.title_fallback.clone();
    let (host, form) = crate::FormHost::new(config);

    let mut shell = Shell::default();
    shell.menu_background = form.menu_pane_background.clone();
    let side_menu_ctrl = root_menu.as_ref().map(|(id, _)| id.clone());
    let side_menu = side_menu_ctrl
        .as_deref()
        .and_then(|id| form.find_control(id));
    // 049 — the designed `Collapsed` is where the application OPENS; once the
    // operator has worked the ☰ themselves, their remembered choice wins (R9).
    shell.collapsed = side_menu.map(|c| c.side_menu_collapsed()).unwrap_or(false);
    let state_path = shell_state_path(&form.name);
    if let Some(p) = &state_path {
        if let Some(c) = load_collapsed_from(p) {
            shell.collapsed = c;
        }
    }
    // 049 — the sidebar's own FullHeight property decides the layout order.
    // Absent (a form written before the property existed) means on.
    shell.full_height = side_menu.map(|c| c.side_menu_full_height()).unwrap_or(true);
    // How the pane paints menu-item icons (None | Shadow | Neumorphic).
    shell.icon_effect = side_menu
        .and_then(|c| c.get_prop("IconEffect"))
        .map(|v| v.as_str().to_owned())
        .unwrap_or_else(|| "None".to_owned());
    // The designed control travels with the shell so the MenuPane paints in
    // the application's own colours, title and profile card.
    shell.side_ctrl = side_menu.cloned();
    // The breadcrumb frame is the rail's chrome, so the rail sizes and colours
    // it: `BreadcrumbHeight` (default 28) and `BreadcrumbBackgroundColor`
    // (empty = follow the ContentPane's backdrop, as it always has).
    if let Some(side) = side_menu {
        shell.breadcrumb_height = cobolt_forms::breadcrumb::height_of(side);
        shell.breadcrumb_bg = side
            .breadcrumb_background()
            .map(|hex| cobolt_forms::paint::parse_color(&hex));
    }
    // 049 R38 — the Open pane is as wide as the developer DREW the rail. The
    // shell used a fixed 220 regardless, so the ContentPane started 20px past
    // where the form was laid out for and every control in it sat that much
    // off. Collapsed stays the icon rail: an icon-only strip is not a width
    // the developer chose, it is what the state means.
    if let Some(w) = side_menu.map(|c| c.rect.w).filter(|w| *w > 0) {
        shell.menu_open_width = w as f32;
    }
    // What a translucent rail colour composites over — the SAME backdrop the
    // ContentPane paints, so the rail and the form agree on the application's
    // background the way they do on the designer canvas.
    shell.form_backdrop = Some(cobolt_forms::render::backdrop_color(
        &form.background_color,
        form.transparency,
    ));
    if let Some((_, def)) = root_menu {
        shell.mount_root_menu(&form.name, def);
    }
    let mut chain = NavChain::default();
    chain.push(NavEntry {
        form_object: form.name.trim().to_ascii_uppercase(),
        label: if form.title.trim().is_empty() {
            form.name.clone()
        } else {
            form.title.clone()
        },
        preserve_on_replace: false,
        resident: Box::new(ChannelResident {
            form_object: form.name.trim().to_ascii_uppercase(),
            ev_tx: ev_tx.clone(),
        }),
    });

    let title = {
        let designed = form.title.trim();
        if designed.is_empty() {
            title_fallback
        } else {
            designed.to_owned()
        }
    };
    // The window opens at the size the form was DESIGNED at — a shell window
    // used to open at a fixed 1100x700 whatever the developer drew, so a form
    // wider than that was clipped on its first frame and a narrower one sat in
    // a window of empty pane. The width is the form's own (the rail's column
    // plus the content beside it, which is exactly what the developer laid
    // out), narrowed when the rail opens collapsed; the height adds the
    // breadcrumb, which is chrome the shell puts OUTSIDE the form.
    let designed = shell_window_size(
        form.width as f32,
        form.height as f32,
        shell.menu_open_width,
        shell.menu_pane_width(),
        // A FullHeight rail's frame OVERLAYS the form's top band, so it costs
        // the window nothing; a panel above the window still needs its own.
        if shell.full_height {
            0.0
        } else {
            shell.breadcrumb_height
        },
    );
    let viewport = egui::ViewportBuilder::default()
        .with_title(&title)
        .with_inner_size([designed.x, designed.y])
        .with_resizable(true)
        // R43 — the shell window carries alpha; the chrome paints itself.
        .with_transparent(true);
    let native_options = crate::native_options(viewport);
    let app = ShellApp {
        shell,
        chain,
        host,
        side_menu_ctrl,
        state_path,
        input_tx,
        ev_tx,
        form_req_tx,
    };
    let _ = eframe::run_native(
        &title,
        native_options,
        Box::new(move |cc| {
            cc.egui_ctx
                .set_fonts(cobolt_forms::fonts::base_font_definitions());
            Ok(Box::new(app) as Box<dyn eframe::App>)
        }),
    );
}

/// The shell window's eframe app: Shell chrome + the main form's [`FormHost`]
/// in the ContentPane.
struct ShellApp {
    shell: Shell,
    chain: NavChain,
    host: crate::FormHost,
    side_menu_ctrl: Option<String>,
    state_path: Option<std::path::PathBuf>,
    input_tx: std::sync::mpsc::Sender<cobolt_runtime::channels::StateUpdate>,
    ev_tx: std::sync::mpsc::Sender<cobolt_runtime::channels::FormEvent>,
    /// 051 R18 — the shell's own door into the supervisor: the standalone
    /// menu actions submit OpenForm requests here (drained by the host's
    /// frame like any interpreter's), with the shell as caller.
    form_req_tx: std::sync::mpsc::Sender<cobolt_runtime::form_host::FormRequest>,
}

impl ShellApp {
    /// 051 R18/R19 — drain and perform this frame's menu activations. The
    /// UI thread never blocks: a Sync open is modal through the supervisor's
    /// modal tracking (the shell's face disables while the child lives), not
    /// through a blocked thread.
    fn process_menu_clicks(&mut self) {
        for click in self.shell.take_menu_clicks() {
            match click.action.as_deref() {
                // Home — the shell's OWN content pane, back on screen, with
                // no form opened to provide it. The forms that were on the
                // pane are parked, not destroyed, so nothing else on screen
                // notices: child windows keep running, and returning to a
                // parked form revives that instance rather than restarting
                // it. Already home ⇒ nothing at all, so a second click does
                // not deactivate and reactivate the main form.
                Some("home") => {
                    if self.chain.len() > 1 {
                        // The detail level named the data on the form that is
                        // leaving the pane; it goes with it.
                        self.shell.detail = None;
                        self.chain.park_to_root();
                        // The contextual slot belongs to whatever occupies
                        // the pane; at the root there is nothing to show —
                        // the same rule a breadcrumb click on segment 0
                        // follows.
                        self.shell.mount_contextual_menu(None);
                        self.host.show_occupant(None);
                    }
                }
                Some(a) if a.starts_with("open-form:") => {
                    // 051 R10/R11 — the embedded door, for real: the target
                    // loads into the ContentPane as its own program instance;
                    // the outgoing occupant deactivates (parking when its
                    // load asked to be preserved, destroyed otherwise) and
                    // the breadcrumb follows the chain.
                    let target = a
                        .split_once(':')
                        .map(|(_, t)| t.trim().to_string())
                        .unwrap_or_default();
                    if target.is_empty() {
                        eprintln!(
                            "shell: menu item '{}' has an open-form action with no target",
                            click.item_id
                        );
                        continue;
                    }
                    let target_upper = target.to_ascii_uppercase();
                    if self
                        .chain
                        .current()
                        .map(|e| e.form_object == target_upper)
                        .unwrap_or(false)
                    {
                        continue; // already displayed
                    }
                    let occ_ev_tx = match self.host.ensure_occupant(&target) {
                        Ok(tx) => tx,
                        Err(e) => {
                            // R15 — visible, never silent.
                            println!("Runtime error: cannot open form '{target}': {e}");
                            eprintln!("shell: open-form '{target}' failed: {e}");
                            continue;
                        }
                    };
                    let entry = NavEntry {
                        form_object: target_upper.clone(),
                        // R21 — the form's designed TITLE names it, exactly as
                        // the main form's own segment does. The chain used to
                        // carry the form OBJECT name here, so a shell whose
                        // root read "Main Menu" pointed at "inner-form1".
                        label: self
                            .host
                            .occupant_label(&target_upper)
                            .unwrap_or_else(|| target.clone()),
                        // Its own fate is decided by whichever click later
                        // navigates away from it (049 R24).
                        preserve_on_replace: false,
                        resident: Box::new(ChannelResident {
                            form_object: target_upper.clone(),
                            ev_tx: occ_ev_tx,
                        }),
                    };
                    // The detail level belonged to the form leaving the pane;
                    // the incoming one starts with a clean crumb and sets its
                    // own if it wants one.
                    self.shell.detail = None;
                    // The FIRST load stacks on the main form; after that a
                    // menu click is a SIBLING load (049 R25) — the displayed
                    // form is replaced, parked when THIS click asked to
                    // preserve it, destroyed otherwise.
                    let mut destroyed: Vec<String> = Vec::new();
                    if self.chain.len() <= 1 {
                        // A form parked by Home is REVIVED rather than
                        // re-entered, so its storage survives the round trip.
                        if !self.chain.push_parked(&target_upper) {
                            self.chain.push(entry);
                        }
                    } else {
                        let outgoing = self.chain.current().map(|e| e.form_object.clone());
                        self.chain.mark_top_preserve(click.preserve_previous_form);
                        self.chain.replace_top(entry);
                        if !click.preserve_previous_form {
                            if let Some(gone) = outgoing {
                                destroyed.push(gone);
                            }
                        }
                    }
                    self.host.retire_occupants(&destroyed);
                    self.host.show_occupant(Some(&target_upper));
                }
                Some(a)
                    if a.starts_with("open-standalone-sync:")
                        || a.starts_with("open-standalone-async:") =>
                {
                    let sync = a.starts_with("open-standalone-sync:");
                    let target = a
                        .split_once(':')
                        .map(|(_, t)| t.trim().to_string())
                        .unwrap_or_default();
                    if target.is_empty() {
                        eprintln!(
                            "shell: menu item '{}' has a standalone action with no \
                             target form",
                            click.item_id
                        );
                        continue;
                    }
                    // The reply is the COBOL caller's affordance; a menu click
                    // has no blocked flow to resume, so it is dropped — the
                    // supervisor's send into it simply fizzles.
                    let (rtx, _rrx) = std::sync::mpsc::channel();
                    let _ = self.form_req_tx.send(
                        cobolt_runtime::form_host::FormRequest::OpenForm {
                            caller: cobolt_runtime::form_host::ROOT_HANDLE.into(),
                            form_id: target,
                            sync,
                            window_state: None,
                            x: None,
                            y: None,
                            width: None,
                            height: None,
                            // 051 R19 — Sync is implicitly modal (operator).
                            modal: sync,
                            reply: rtx,
                        },
                    );
                }
                Some("close-application") => {
                    let _ = self
                        .ev_tx
                        .send(cobolt_runtime::channels::FormEvent::quit());
                }
                _ => {
                    // `event` items dispatch to the SideMenu control's
                    // handler; the item id travels as a property first.
                    if let Some(ctrl) = &self.side_menu_ctrl {
                        let _ = self.input_tx.send(
                            cobolt_runtime::channels::StateUpdate::new(
                                ctrl.clone(),
                                "SelectedItemId".to_string(),
                                click.item_id.clone(),
                            ),
                        );
                        let _ = self.ev_tx.send(cobolt_runtime::channels::FormEvent::new(
                            ctrl.clone(),
                            "onMenuItemClick",
                        ));
                    }
                }
            }
        }
    }
}

/// The window width that keeps the ContentPane the same size across a rail
/// toggle.
///
/// Opening the rail takes its width out of the ContentPane, which clips
/// whatever the developer laid out at the right-hand edge; collapsing it hands
/// the width back and leaves a band of nothing. So the WINDOW absorbs the
/// change instead of the content: it grows by the rail's width on open and
/// gives it back on collapse, returning to the size it had. The pane the
/// developer designed against is the one thing that never moves.
///
/// Clamped to [`MIN_SHELL_WIDTH`] so collapsing can never shrink the window to
/// nothing on a monitor narrower than the rail.
pub fn shell_width_for_pane(current: f32, open_w: f32, collapsed_w: f32, opening: bool) -> f32 {
    let delta = (open_w - collapsed_w).max(0.0);
    if opening {
        current + delta
    } else {
        (current - delta).max(MIN_SHELL_WIDTH)
    }
}

/// The narrowest the shell will resize ITSELF to. The operator may still drag
/// it smaller — this only bounds what a rail toggle does on its own.
pub const MIN_SHELL_WIDTH: f32 = 480.0;

/// The size a shell window opens at, for a form designed `form_w` x `form_h`.
///
/// The form's designed width already spans the rail's column AND the content
/// beside it, so it IS the open-rail window width; a rail that opens collapsed
/// gives the difference back.
///
/// `crumb_extra` is the height the breadcrumb frame takes OUT of the form: 0
/// when it overlays the form's own top band (a FullHeight rail — the designer
/// canvas draws it that way, and controls may sit over it), and the frame's
/// height when it is a panel above the whole window (FullHeight off), where
/// without it the bottom of the form would be under the window edge on the
/// very first frame.
pub fn shell_window_size(
    form_w: f32,
    form_h: f32,
    open_w: f32,
    current_pane_w: f32,
    crumb_extra: f32,
) -> egui::Vec2 {
    let content_w = (form_w - open_w).max(1.0);
    egui::Vec2::new(
        (current_pane_w + content_w).max(MIN_SHELL_WIDTH),
        (form_h + crumb_extra).max(1.0),
    )
}

impl ShellApp {
    fn persist_collapsed(&self) {
        if let Some(p) = &self.state_path {
            let _ = save_collapsed_to(p, self.shell.collapsed);
        }
    }

    /// Move the window by the rail's width so the ContentPane keeps its own.
    /// `opening` is the direction the rail just went.
    fn resize_window_for_pane(&self, ctx: &egui::Context, opening: bool) {
        let Some(size) = ctx.input(|i| i.viewport().inner_rect.map(|r| r.size())) else {
            return;
        };
        let width = shell_width_for_pane(
            size.x,
            self.shell.menu_open_width,
            self.shell.menu_collapsed_width,
            opening,
        );
        if (width - size.x).abs() < 0.5 {
            return;
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(width, size.y)));
    }

    /// The form object currently on the pane — the chain's last entry.
    fn displayed_form(&self) -> Option<String> {
        self.chain.current().map(|e| e.form_object.clone())
    }

    /// A COBOL-set breadcrumb detail level (`me::"SetBreadcrumbDetail"`).
    /// Accepted only from the DISPLAYED form: a crumb is one step under a
    /// name, and a form that is not on the pane has no name up there to hang
    /// it from.
    fn apply_crumb_detail(&mut self, form_object: &str, text: Option<String>) {
        if self.displayed_form().as_deref() != Some(form_object) {
            return;
        }
        self.shell.detail = text;
    }

    /// A click on the displayed form's OWN segment, with a detail level after
    /// it: start that form over.
    ///
    /// The form gets the last word. While its `PreventReset` is on — the
    /// guard its COBOL sets whenever it is holding something worth losing —
    /// nothing is reset and `onResetRejected` fires instead, so the
    /// application can say why.
    ///
    /// Allowed, the reset is a REBUILD: the displayed occupant is destroyed
    /// (its `onDestroy` runs, files close, storage is released) and a fresh
    /// instance takes its place, blank as on the day it was first opened. The
    /// shell's OWN main form has no separate instance to rebuild — restarting
    /// it would restart the application — so it is sent `onReset` and does its
    /// own housekeeping.
    fn reset_displayed_form(&mut self) {
        let Some(form) = self.displayed_form() else {
            return;
        };
        let guarded = self
            .host
            .published_form_prop(Some(&form), "PreventReset")
            .map(|v| {
                let v = v.trim();
                !(v.is_empty() || v == "0" || v.eq_ignore_ascii_case("false"))
            })
            .unwrap_or(false);
        if guarded {
            self.host.notify_form(Some(&form), "onResetRejected");
            return;
        }
        // The detail level described the data that is going away.
        self.shell.detail = None;
        if self.chain.len() <= 1 {
            // The shell's own form: no second instance to swap in.
            self.host.notify_form(Some(&form), "onReset");
            return;
        }
        // Destroy the occupant (onDestroy through its Resident), retire its
        // interpreter and handle, then build it again from scratch. Its
        // PARENT is untouched throughout — it never stopped being an ancestor.
        let destroyed = self.chain.pop_to(self.chain.len() - 2);
        self.host.retire_occupants(&destroyed);
        match self.host.ensure_occupant(&form) {
            Ok(ev_tx) => {
                let label = self
                    .host
                    .occupant_label(&form)
                    .unwrap_or_else(|| form.clone());
                self.chain.push_restarted(NavEntry {
                    form_object: form.clone(),
                    label,
                    preserve_on_replace: false,
                    resident: Box::new(ChannelResident {
                        form_object: form.clone(),
                        ev_tx,
                    }),
                });
                self.host.show_occupant(Some(&form));
            }
            Err(e) => {
                // R15 — visible, never silent. The pane falls back to the
                // form the reset left displayed (its parent).
                println!("Runtime error: cannot restart form '{form}': {e}");
                eprintln!("shell: reset of '{form}' failed: {e}");
                if let Some(parent) = self.displayed_form() {
                    self.host.show_occupant(Some(&parent));
                }
            }
        }
    }
}

impl eframe::App for ShellApp {
    // R43 — the surface behind the panes is fully transparent; every visible
    // pixel is an explicit paint (chrome, or the form's backdrop).
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }

    fn ui(&mut self, root_ui: &mut Ui, frame: &mut eframe::Frame) {
        let _ = frame;
        // 051 R19 — while a Sync-opened (modal) child window lives, the WHOLE
        // shell face waits: chrome, breadcrumb and pane alike.
        if self.host.root_modal_blocked() {
            root_ui.disable();
        }
        let shell = &mut self.shell;
        let host = &mut self.host;
        // R21 — the chain the strip renders. The shell paints it (and the
        // sidebar's Open/Collapsed control at its head) through the shared
        // renderer; the header pane of the rail owns the other toggle, so an
        // empty menu is still collapsible from either.
        shell.breadcrumb = self
            .chain
            .segments()
            .into_iter()
            .map(|(_, label)| label)
            .collect();
        shell.show_with_host(root_ui, |_ui| {}, host);
        let crumb_click = shell.take_breadcrumb_click();
        let reset_click = shell.take_reset_request();
        // A rail toggle moves the WINDOW, not the ContentPane's width — from
        // either affordance, and from COBOL. The window grows by the rail on
        // open and returns to its old size on collapse, so the content the
        // developer laid out is never clipped by the frame nor left beside a
        // band of nothing.
        let ctx = root_ui.ctx().clone();
        if self.shell.take_toggle_request() {
            self.shell.collapsed = !self.shell.collapsed;
            self.resize_window_for_pane(&ctx, !self.shell.collapsed);
            self.persist_collapsed();
        }
        // R44 — COBOL drove the pane through the supervisor.
        if let Some(collapsed) = self.host.take_menu_pane_request() {
            if collapsed != self.shell.collapsed {
                self.shell.collapsed = collapsed;
                self.resize_window_for_pane(&ctx, !collapsed);
                self.persist_collapsed();
            }
        }
        // A COBOL-set breadcrumb detail level (`me::"SetBreadcrumbDetail"`).
        if let Some((form_object, text)) = self.host.take_breadcrumb_detail() {
            self.apply_crumb_detail(&form_object, text);
        }
        // Menu activations — each action performed by its own arm.
        self.process_menu_clicks();
        // 051 R12/R22 — a breadcrumb click truncates the chain: everything
        // below the clicked segment is destroyed deepest-first, and the
        // segment's own form returns to the pane.
        if let Some(ix) = crumb_click {
            if ix + 1 < self.chain.len() {
                // The detail level hung off the form being left behind.
                self.shell.detail = None;
                let destroyed = self.chain.pop_to(ix);
                self.host.retire_occupants(&destroyed);
                if ix == 0 {
                    self.host.show_occupant(None);
                } else if let Some(target) =
                    self.chain.current().map(|e| e.form_object.clone())
                {
                    self.host.show_occupant(Some(&target));
                }
            }
        }
        // The displayed form's own segment, clicked with a detail level after
        // it: start that form over — unless it says it would lose data.
        if reset_click {
            self.reset_displayed_form();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(size: Vec2) -> egui::RawInput {
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, size)),
            ..Default::default()
        }
    }

    /// One headless shell frame with content larger than both panes, so both
    /// CAN scroll.
    fn frame(shell: &mut Shell, ctx: &egui::Context, input: egui::RawInput) -> ShellLayout {
        let mut out = None;
        let mut full = ctx.run_ui(input, |root_ui| {
            out = Some(shell.show(
                root_ui,
                |ui| {
                    ui.allocate_space(Vec2::new(10.0, 2000.0));
                },
                |ui| {
                    ui.allocate_space(Vec2::new(3000.0, 3000.0));
                },
            ));
        });
        full.textures_delta.clear();
        out.expect("shell ran")
    }

    /// A frame that clicks at `pos` (press + release in the same frame, which
    /// is what egui needs to report `clicked()`).
    fn click_at(shell: &mut Shell, ctx: &egui::Context, size: Vec2, pos: egui::Pos2) {
        let mut input = raw(size);
        input.events = vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            },
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            },
        ];
        frame(shell, ctx, input);
    }

    /// The sidebar collapses and opens with NOTHING in its menu: the toggle
    /// belongs to the pane, not to the menu content. An application whose
    /// menu is still empty must not trap the operator in an open sidebar.
    #[test]
    fn pane_toggles_with_an_empty_menu_049() {
        let ctx = egui::Context::default();
        let mut shell = Shell::default();
        let size = Vec2::new(1000.0, 700.0);

        // No mount_root_menu, no mount_contextual_menu — the menu is empty.
        let first = frame(&mut shell, &ctx, raw(size));
        assert!(
            shell.mounted().0.is_none() && shell.mounted().1.is_none(),
            "precondition: no menu is mounted"
        );
        let toggle = shell
            .toggle_rect()
            .expect("the toggle is drawn even with an empty menu");
        assert!(
            first.menu_rect.contains(toggle.center()),
            "the toggle sits on the MenuPane itself, not in the breadcrumb"
        );

        // Open → Collapsed.
        assert!(!shell.collapsed);
        click_at(&mut shell, &ctx, size, toggle.center());
        assert!(
            shell.take_toggle_request(),
            "clicking the toggle asks the owner to flip the pane"
        );
        shell.collapsed = true;

        // Collapsed → Open: still reachable on the narrow rail.
        let narrow = frame(&mut shell, &ctx, raw(size));
        let toggle = shell.toggle_rect().expect("the rail keeps the toggle");
        assert!(
            narrow.menu_rect.width() < MENU_PANE_OPEN_WIDTH,
            "precondition: the pane is collapsed"
        );
        assert!(
            narrow.menu_rect.contains(toggle.center()),
            "the collapsed rail still carries its toggle"
        );
        click_at(&mut shell, &ctx, size, toggle.center());
        assert!(
            shell.take_toggle_request(),
            "the collapsed rail's toggle opens the pane again"
        );

        eprintln!(
            "049 FullHeight/toggle — empty menu: toggle drawn on the pane in \
             BOTH states (open w={:.0}, collapsed w={:.0}), 2/2 clicks \
             requested a flip, 0 menu items mounted",
            MENU_PANE_OPEN_WIDTH,
            narrow.menu_rect.width()
        );
    }

    /// `FullHeight` is a layout ORDER, not a size: on, the sidebar owns the
    /// window's whole vertical extent and the breadcrumb starts at its edge;
    /// off, the breadcrumb spans the width and the sidebar fills what is
    /// left. Either way the pane fills its own column, collapsed or open.
    #[test]
    fn full_height_decides_who_owns_the_top_left_corner_049() {
        let ctx = egui::Context::default();
        let size = Vec2::new(1000.0, 700.0);

        for collapsed in [false, true] {
            let mut on = Shell::default();
            on.collapsed = collapsed;
            let l = frame(&mut on, &ctx, raw(size));
            assert!(
                (l.menu_rect.height() - size.y).abs() < 1.0,
                "FullHeight on ({collapsed:?} collapsed): the sidebar spans the \
                 whole window height, got {}",
                l.menu_rect.height()
            );
            assert!(
                l.breadcrumb_rect.min.x >= l.menu_rect.max.x - 1.0,
                "FullHeight on: the breadcrumb starts at the sidebar's edge"
            );

            let mut off = Shell::default();
            off.collapsed = collapsed;
            off.full_height = false;
            let l = frame(&mut off, &ctx, raw(size));
            assert!(
                (l.breadcrumb_rect.width() - size.x).abs() < 1.0,
                "FullHeight off: the breadcrumb spans the whole width, got {}",
                l.breadcrumb_rect.width()
            );
            assert!(
                l.menu_rect.min.y >= l.breadcrumb_rect.max.y - 1.0,
                "FullHeight off: the sidebar starts below the breadcrumb"
            );
            assert!(
                (l.menu_rect.max.y - size.y).abs() < 1.0,
                "FullHeight off: the sidebar still fills the height beneath it, \
                 bottom={}",
                l.menu_rect.max.y
            );
        }

        eprintln!(
            "049 FullHeight — 2 states (open, collapsed) x 2 settings: on ⇒ \
             sidebar height = window height {:.0} and breadcrumb inset; off ⇒ \
             breadcrumb width = window width {:.0} and sidebar below it, still \
             reaching the window bottom (4/4)",
            size.y, size.x
        );
    }

    /// AC20 — widening the window adds the whole delta to the ContentPane;
    /// the MenuPane's width is unchanged (R38).
    #[test]
    fn window_resize_is_absorbed_by_the_content_pane() {
        let ctx = egui::Context::default();
        let mut shell = Shell::default();
        let narrow = frame(&mut shell, &ctx, raw(Vec2::new(1000.0, 700.0)));
        let wide = frame(&mut shell, &ctx, raw(Vec2::new(1400.0, 700.0)));

        assert_eq!(
            narrow.menu_rect.width(),
            wide.menu_rect.width(),
            "R38: the MenuPane must not follow a window resize"
        );
        let delta_content = wide.content_rect.width() - narrow.content_rect.width();
        assert!(
            (delta_content - 400.0).abs() < 1.0,
            "AC20: the ContentPane absorbs the whole 400px delta, got {delta_content}"
        );
        // R4 — three regions, disjoint: the breadcrumb sits above the content,
        // to the right of the menu.
        assert!(wide.breadcrumb_rect.min.x >= wide.menu_rect.max.x - 1.0);
        assert!(wide.content_rect.min.y >= wide.breadcrumb_rect.max.y - 1.0);

        println!(
            "049 AC20 — 1000→1400px: MenuPane {:.0}px in both frames, \
             ContentPane +{:.0}px (absorbed the full delta); regions disjoint \
             (menu | breadcrumb above content)",
            wide.menu_rect.width(),
            delta_content
        );
    }

    /// AC19 — scrolling one pane leaves the other's offset untouched, both
    /// directions.
    #[test]
    fn pane_scrolls_are_independent() {
        let ctx = egui::Context::default();
        let mut shell = Shell::default();
        let size = Vec2::new(1000.0, 700.0);

        // Frame 1: lay out (offsets 0/0).
        let l0 = frame(&mut shell, &ctx, raw(size));
        assert_eq!(l0.menu_scroll, Vec2::ZERO);
        assert_eq!(l0.content_scroll, Vec2::ZERO);

        // Frame 2: wheel over the CONTENT pane.
        let mut input = raw(size);
        input
            .events
            .push(egui::Event::PointerMoved(l0.content_rect.center()));
        input.events.push(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: Vec2::new(0.0, -120.0),
            phase: egui::TouchPhase::Move,
            modifiers: Default::default(),
        });
        frame(&mut shell, &ctx, input);
        // Frame 3: read the settled offsets.
        let l1 = frame(&mut shell, &ctx, raw(size));
        assert!(
            l1.content_scroll.y > 0.0,
            "the ContentPane scrolled: {:?}",
            l1.content_scroll
        );
        assert_eq!(
            l1.menu_scroll,
            Vec2::ZERO,
            "the MenuPane must not move when the ContentPane scrolls"
        );

        // Frame 4: wheel over the MENU pane.
        let mut input = raw(size);
        input
            .events
            .push(egui::Event::PointerMoved(l0.menu_rect.center()));
        input.events.push(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: Vec2::new(0.0, -80.0),
            phase: egui::TouchPhase::Move,
            modifiers: Default::default(),
        });
        frame(&mut shell, &ctx, input);
        let l2 = frame(&mut shell, &ctx, raw(size));
        assert!(
            l2.menu_scroll.y > 0.0,
            "the MenuPane scrolled: {:?}",
            l2.menu_scroll
        );
        assert_eq!(
            l2.content_scroll, l1.content_scroll,
            "the ContentPane's offset must be untouched by a MenuPane scroll"
        );

        println!(
            "049 AC19 — content wheel: content {:.0}px / menu 0px; menu wheel: \
             menu {:.0}px / content unchanged at {:.0}px (both directions \
             independent)",
            l1.content_scroll.y, l2.menu_scroll.y, l2.content_scroll.y
        );
    }

    /// **A form LOADED into the pane is placed in the pane.** It was drawn
    /// from the window's top-left instead — over the menu rail, offset by
    /// nothing but the breadcrumb band — while the shell's own form, on the
    /// same pane, sat where it belonged (operator, 2026-08-20).
    ///
    /// The two took different roads: the shell form goes through a
    /// `CentralPanel`, which consumes what the panels left; the occupant was
    /// placed from the root `Ui`'s FULL rect, and that `Ui` is the very
    /// surface those panels were added to. So the assertion is against the
    /// shell's own `content_rect` — one number, from the shell's bookkeeping,
    /// not a hand-computed guess at where the rail ends.
    #[test]
    fn a_loaded_form_is_placed_in_the_content_pane_not_the_window() {
        use crate::host::{FormHostConfig, FormSource, NoHooks, Surface};
        use std::collections::HashMap;
        use std::sync::atomic::{AtomicBool, AtomicUsize};
        use std::sync::{mpsc, Arc};

        fn program() -> cobolt_ast::program::Program {
            let src = "\
IDENTIFICATION DIVISION.\nPROGRAM-ID. CHILD.\nPROCEDURE DIVISION.\n    STOP RUN.\n";
            cobolt_parser::parse(cobolt_lexer::tokenize(src, cobolt_lexer::SourceFormat::Free))
                .program
                .expect("parses")
        }

        let form = cobolt_forms::Form::new("MAIN-FORM", "Main", 800, 600);
        let (ev_tx, _ev_rx) = mpsc::channel();
        let (input_tx, _input_rx) = mpsc::channel();
        let (_state_tx, state_rx) = mpsc::channel();
        let (_display_tx, display_rx) = mpsc::channel();
        let (_form_req_tx, form_req_rx) = mpsc::channel();
        let (closed_tx, _closed_rx) = mpsc::channel();
        let source: FormSource = Box::new(|id: &str| {
            let up = id.trim().to_ascii_uppercase();
            Ok((
                cobolt_forms::Form::new(up.as_str(), up.as_str(), 400, 300),
                program(),
            ))
        });
        let (mut host, _f) = crate::FormHost::new(FormHostConfig {
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
            form_req_tx: _form_req_tx.clone(),
            form_source: Some(source),
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

        host.ensure_occupant("CRM").expect("CRM builds");
        host.show_occupant(Some("CRM"));

        let ctx = egui::Context::default();
        let mut shell = Shell::default();
        let mut run = |shell: &mut Shell, host: &mut crate::FormHost| -> ShellLayout {
            let mut out = None;
            let mut full = ctx.run_ui(raw(Vec2::new(1000.0, 700.0)), |root_ui| {
                out = Some(shell.show_with_host(root_ui, |_ui| {}, host));
            });
            full.textures_delta.clear();
            out.expect("shell ran")
        };

        let open = run(&mut shell, &mut host);
        let placed = host
            .last_occupant_rect()
            .expect("an occupant owns the pane, so it was placed");

        assert!(
            (placed.min.x - open.content_rect.min.x).abs() < 1.0,
            "the loaded form starts at the ContentPane's left edge \
             ({}), not the window's ({}) — it was drawn over the rail",
            open.content_rect.min.x,
            placed.min.x
        );
        assert!(
            placed.min.x >= MENU_PANE_OPEN_WIDTH - 1.0,
            "…which is right of the open rail ({MENU_PANE_OPEN_WIDTH}px), \
             not at the window origin: {placed:?}"
        );
        assert!(
            (placed.max.x - open.content_rect.max.x).abs() < 1.0,
            "and it ends where the pane ends: {placed:?} vs {:?}",
            open.content_rect
        );

        // …and it travels with the pane, exactly as the shell's own form does.
        shell.collapsed = true;
        let collapsed = run(&mut shell, &mut host);
        let moved = host.last_occupant_rect().expect("still placed");
        let shift = placed.min.x - moved.min.x;
        let expect = MENU_PANE_OPEN_WIDTH - MENU_PANE_COLLAPSED_WIDTH;
        assert!(
            (shift - expect).abs() < 1.0,
            "collapsing the rail moves the loaded form left by the rail delta: \
             shift {shift}, expect {expect}"
        );
        assert!(
            (moved.min.x - collapsed.content_rect.min.x).abs() < 1.0,
            "and it is still anchored to the pane, not to the window"
        );
    }

    /// AC4 — a hosted form travels with the pane edge on collapse, at its
    /// designed size. (The form anchors at the ContentPane origin — the
    /// render engine draws at `ui.min_rect().min` by contract — so the
    /// origin shift IS the form's shift.)
    #[test]
    fn form_travels_with_the_pane_edge_at_designed_size() {
        use crate::host::{FormHostConfig, NoHooks, Surface};
        use std::collections::HashMap;
        use std::sync::atomic::{AtomicBool, AtomicUsize};
        use std::sync::{mpsc, Arc};

        let form = cobolt_forms::Form::new("EMB", "Embedded", 300, 200);
        let (ev_tx, _ev_rx) = mpsc::channel();
        let (input_tx, _input_rx) = mpsc::channel();
        let (_state_tx, state_rx) = mpsc::channel();
        let (_display_tx, display_rx) = mpsc::channel();
        let (_form_req_tx, form_req_rx) = mpsc::channel();
        let (closed_tx, _closed_rx) = mpsc::channel();
        let (mut host, _form) = crate::FormHost::new(FormHostConfig {
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
            form_req_tx: _form_req_tx.clone(),
            form_source: None,
            child_theme: None,
            child_interpreter_setup: None,
            shared_rust_bridge: None,
            fx_entrance: cobolt_forms::window_fx::FxSpec::parse("fade:2000:linear"),
            fx_exit: cobolt_forms::window_fx::FxSpec::default(),
            fx_restore: false,
            theme_pack: None,
            surface_theme: cobolt_forms::surface_theme::liquid_glass(),
            icon_path: None,
            title_fallback: String::new(),
            hooks: Box::new(NoHooks),
            surface: Surface::Pane,
        });

        let ctx = egui::Context::default();
        let mut shell = Shell::default();
        let mut run = |shell: &mut Shell, host: &mut crate::FormHost| -> ShellLayout {
            let mut out = None;
            let mut full = ctx.run_ui(raw(Vec2::new(1000.0, 700.0)), |root_ui| {
                out = Some(shell.show_with_host(
                    root_ui,
                    |ui| {
                        ui.label("menu");
                    },
                    host,
                ));
            });
            full.textures_delta.clear();
            out.expect("shell ran")
        };

        let open = run(&mut shell, &mut host);
        shell.collapsed = true;
        let collapsed = run(&mut shell, &mut host);

        // open.min.x − collapsed.min.x = +delta ⇔ collapsing moved the pane
        // origin (and the form anchored to it) LEFT by the rail delta.
        let shift = open.content_rect.min.x - collapsed.content_rect.min.x;
        let expect = MENU_PANE_OPEN_WIDTH - MENU_PANE_COLLAPSED_WIDTH;
        assert!(
            (shift - expect).abs() < 1.0,
            "the pane origin moves LEFT by the rail delta: shift {shift}, expect {expect}"
        );
        assert_eq!(
            host.designed_size(),
            Vec2::new(300.0, 200.0),
            "AC4/R11: the designed size never follows the pane"
        );
        println!(
            "049 AC4 — collapse moved the ContentPane origin {:.0}px left \
             (rail delta {:.0}); hosted form stayed at its designed 300x200 \
             (Pane surface, entrance spec inert)",
            shift, expect
        );
    }

    /// AC21/AC5 — a form larger than the pane scrolls inside it while the
    /// pane-fixed backdrop stays put, and the backdrop rect is PANE-sized
    /// (never form-sized), for a bigger AND a smaller form (R12/R13/R41).
    #[test]
    fn backdrop_stays_pane_fixed_while_the_form_scrolls() {
        use crate::host::{FormHostConfig, NoHooks, Surface};
        use std::collections::HashMap;
        use std::sync::atomic::{AtomicBool, AtomicUsize};
        use std::sync::{mpsc, Arc};

        let mk_host = |w: u32, h: u32| {
            let mut form = cobolt_forms::Form::new("EMB", "Embedded", w, h);
            form.background_color = "2E3138FF".into();
            let (ev_tx, _ev_rx) = mpsc::channel();
            let (input_tx, _input_rx) = mpsc::channel();
            let (_state_tx, state_rx) = mpsc::channel();
            let (_display_tx, display_rx) = mpsc::channel();
            let (_form_req_tx, form_req_rx) = mpsc::channel();
            let (closed_tx, _closed_rx) = mpsc::channel();
            let (host, _f) = crate::FormHost::new(FormHostConfig {
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
                form_req_tx: _form_req_tx.clone(),
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
            host
        };

        let ctx = egui::Context::default();
        let mut shell = Shell::default();
        let size = Vec2::new(1000.0, 700.0);
        let mut run = |shell: &mut Shell,
                       host: &mut crate::FormHost,
                       input: egui::RawInput|
         -> ShellLayout {
            let mut out = None;
            let mut full = ctx.run_ui(input, |root_ui| {
                out = Some(shell.show_with_host(
                    root_ui,
                    |ui| {
                        ui.label("menu");
                    },
                    host,
                ));
            });
            full.textures_delta.clear();
            out.expect("shell ran")
        };

        // A 3000x3000 form in a ~750x670 pane.
        let mut big = mk_host(3000, 3000);
        let l0 = run(&mut shell, &mut big, raw(size));
        let rect0 = big.pane_backdrop_rect().expect("pane backdrop painted");
        assert!(
            (rect0.width() - l0.content_rect.width()).abs() < 1.0
                && rect0.width() < 2999.0,
            "R13: the backdrop rect is PANE-sized, not form-sized: {rect0:?} vs pane {:?}",
            l0.content_rect
        );

        // Wheel over the content: the form scrolls, the backdrop rect stays.
        let mut input = raw(size);
        input
            .events
            .push(egui::Event::PointerMoved(l0.content_rect.center()));
        input.events.push(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: Vec2::new(0.0, -150.0),
            phase: egui::TouchPhase::Move,
            modifiers: Default::default(),
        });
        run(&mut shell, &mut big, input);
        let _l2 = run(&mut shell, &mut big, raw(size));
        let scrolled = big.content_scroll();
        let rect2 = big.pane_backdrop_rect().expect("still painted");
        assert!(
            scrolled.y > 0.0,
            "AC21: the form scrolled inside the pane: {scrolled:?}"
        );
        assert_eq!(
            rect0, rect2,
            "R41: the backdrop rect must not move with the scroll"
        );

        // A 300x200 form: the backdrop still covers the WHOLE pane (R12).
        let mut small = mk_host(300, 200);
        let l3 = run(&mut shell, &mut small, raw(size));
        let rect3 = small.pane_backdrop_rect().expect("painted");
        assert!(
            (rect3.width() - l3.content_rect.width()).abs() < 1.0
                && rect3.width() > 300.0,
            "R12: a small form's backdrop covers the whole pane: {rect3:?}"
        );

        println!(
            "049 AC21/AC5 — 3000x3000 form: backdrop rect {}x{} (pane-sized), \
             scrolled {:.0}px with the rect unmoved; 300x200 form: backdrop \
             covers the full {}x{} pane",
            rect0.width() as i32,
            rect0.height() as i32,
            scrolled.y,
            rect3.width() as i32,
            rect3.height() as i32
        );
    }

    /// T29 — the SAME `Both`-format form through both surfaces: assert ONLY
    /// the documented differences (backdrop ownership + rect, effects), plus
    /// the quantified navigation summary the tasks require.
    #[test]
    fn zz_pane_window_parity_report() {
        use crate::host::{FormHostConfig, NoHooks, Surface};
        use std::collections::HashMap;
        use std::sync::atomic::{AtomicBool, AtomicUsize};
        use std::sync::{mpsc, Arc};

        let mk = |surface: Surface| {
            let mut form = cobolt_forms::Form::new("BOTH-FORM", "Both", 300, 200);
            form.form_format = cobolt_forms::model::FormFormat::Both;
            form.background_color = "2E3138FF".into();
            let (ev_tx, _ev_rx) = mpsc::channel();
            let (input_tx, _input_rx) = mpsc::channel();
            let (_state_tx, state_rx) = mpsc::channel();
            let (_display_tx, display_rx) = mpsc::channel();
            let (_form_req_tx, form_req_rx) = mpsc::channel();
            let (closed_tx, _closed_rx) = mpsc::channel();
            let (host, _f) = crate::FormHost::new(FormHostConfig {
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
                form_req_tx: _form_req_tx.clone(),
                form_source: None,
                child_theme: None,
                child_interpreter_setup: None,
                shared_rust_bridge: None,
                fx_entrance: cobolt_forms::window_fx::FxSpec::parse("fade:2000:linear"),
                fx_exit: cobolt_forms::window_fx::FxSpec::default(),
                fx_restore: false,
                theme_pack: None,
                surface_theme: cobolt_forms::surface_theme::liquid_glass(),
                icon_path: None,
                title_fallback: String::new(),
                hooks: Box::new(NoHooks),
                surface,
            });
            host
        };

        let ctx = egui::Context::default();
        // Window surface, driven directly (the classic host).
        let mut window = mk(Surface::Window);
        let mut full = ctx.run_ui(raw(Vec2::new(1000.0, 700.0)), |root_ui| {
            window.pane_frame(root_ui); // same frame body; Surface gates differ
        });
        full.textures_delta.clear();
        // Pane surface, inside the shell.
        let ctx2 = egui::Context::default();
        let mut shell = Shell::default();
        let mut pane = mk(Surface::Pane);
        let mut full = ctx2.run_ui(raw(Vec2::new(1000.0, 700.0)), |root_ui| {
            shell.show_with_host(root_ui, |_ui| {}, &mut pane);
        });
        full.textures_delta.clear();

        // Documented difference 1 — backdrop ownership: engine (Window) vs
        // pane-fixed (Pane), and the Pane rect is pane-sized.
        assert_eq!(window.pane_backdrop_rect(), None);
        let pane_rect = pane.pane_backdrop_rect().expect("pane paints it");
        assert!(pane_rect.width() > 300.0, "pane-sized, beyond the form");
        // Documented difference 2 — effects: pending on Window, inert on Pane.
        assert!(!window.entrance_done(), "Window: the 2s entrance is playing");
        assert!(pane.entrance_done(), "Pane: no effects (R18)");
        // NOT different: the designed size.
        assert_eq!(window.designed_size(), pane.designed_size());

        // Quantified navigation summary (chain depth / resident count / hop
        // timing), per the tasks' reporting rule.
        let log = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let start = std::time::Instant::now();
        let mut chain = NavChain::default();
        for i in 0..100 {
            chain.push(entry(&format!("F{i}"), false, &log));
        }
        let depth = chain.len();
        let resident = chain.resident_count();
        let destroyed = chain.pop_to(0).len();
        let elapsed = start.elapsed();

        println!(
            "049 T29 parity — differences (exactly as documented): backdrop \
             Window=engine/None vs Pane=fixed {}x{}; effects Window=playing vs \
             Pane=inert; SAME designed size {}x{}. Navigation: depth {} pushed, \
             {} resident, {} destroyed deepest-first, 100 push + 99 destroys in \
             {:.2}ms ({:.1}µs/hop)",
            pane_rect.width() as i32,
            pane_rect.height() as i32,
            window.designed_size().x as i32,
            window.designed_size().y as i32,
            depth,
            resident,
            destroyed,
            elapsed.as_secs_f64() * 1000.0,
            elapsed.as_micros() as f64 / 199.0
        );
    }

    /// An instrumented Resident: every lifecycle call lands in the shared log
    /// as "<form>:<event>", so order and discipline are assertable.
    struct Probe {
        name: String,
        log: std::rc::Rc<std::cell::RefCell<Vec<String>>>,
    }
    impl Resident for Probe {
        fn deactivate(&mut self) {
            self.log.borrow_mut().push(format!("{}:deactivate", self.name));
        }
        fn destroy(&mut self) {
            self.log.borrow_mut().push(format!("{}:destroy", self.name));
        }
    }

    fn entry(
        name: &str,
        preserve: bool,
        log: &std::rc::Rc<std::cell::RefCell<Vec<String>>>,
    ) -> NavEntry {
        NavEntry {
            form_object: name.to_string(),
            label: name.to_string(),
            preserve_on_replace: preserve,
            resident: Box::new(Probe {
                name: name.to_string(),
                log: log.clone(),
            }),
        }
    }

    /// Home — the shell's own content pane returns and NOTHING is destroyed.
    /// This is the whole difference from a breadcrumb click on segment 0,
    /// which destroys everything below it: Home only parks, so the forms that
    /// were on the pane keep their WORKING-STORAGE and every other live form
    /// carries on.
    #[test]
    fn home_parks_the_chain_back_to_the_root_without_destroying_anything() {
        let log = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let mut chain = NavChain::default();
        for name in ["MAIN", "CRM", "SALES"] {
            chain.push(entry(name, false, &log));
        }
        log.borrow_mut().clear(); // the pushes' own deactivations

        chain.park_to_root();

        let events = log.borrow().clone();
        assert!(
            !events.iter().any(|e| e.ends_with(":destroy")),
            "Home must not destroy anything, got {events:?}"
        );
        assert_eq!(
            events,
            vec!["SALES:deactivate".to_string()],
            "only the DISPLAYED form deactivates; SALES's ancestors already had"
        );
        let segs: Vec<String> = chain.segments().into_iter().map(|(f, _)| f).collect();
        assert_eq!(segs, vec!["MAIN".to_string()], "the breadcrumb is just the root");
        assert_eq!(
            chain.resident_count(),
            3,
            "MAIN displayed, CRM and SALES parked — all three still alive"
        );

        // Home again at the root changes nothing at all (no stray deactivate).
        log.borrow_mut().clear();
        chain.park_to_root();
        assert!(log.borrow().is_empty(), "Home at the root is a no-op");

        println!(
            "Home — chain MAIN>CRM>SALES collapsed to MAIN with 0 destroys, \
             3 forms still resident, breadcrumb {segs:?}"
        );
    }

    /// After Home, loading a parked form REVIVES that instance rather than
    /// stacking a second entry for it. Without this the chain would hold an
    /// unreachable parked twin that is never destroyed.
    #[test]
    fn a_form_parked_by_home_is_revived_not_duplicated() {
        let log = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let mut chain = NavChain::default();
        for name in ["MAIN", "CRM"] {
            chain.push(entry(name, false, &log));
        }
        chain.park_to_root();
        assert_eq!(chain.resident_count(), 2);

        assert!(chain.push_parked("CRM"), "CRM was parked and must revive");
        let segs: Vec<String> = chain.segments().into_iter().map(|(f, _)| f).collect();
        assert_eq!(segs, vec!["MAIN".to_string(), "CRM".to_string()]);
        assert_eq!(
            chain.resident_count(),
            2,
            "revived, not duplicated — no orphan left in the parking lot"
        );
        assert!(
            !log.borrow().iter().any(|e| e.ends_with(":destroy")),
            "reviving destroys nothing"
        );

        // A form that was never parked is not claimed from the lot.
        assert!(!chain.push_parked("PAYROLL"));

        println!("Home return — CRM revived from the parking lot, 2 residents, 0 destroys");
    }

    /// AC10 — clicking the CRM segment destroys CUST-LIST then SALES in that
    /// order, remounts CRM's menu into the contextual slot, and CRM's
    /// residency is untouched.
    #[test]
    fn breadcrumb_click_unwinds_and_remounts() {
        use cobolt_forms::menu::{MenuDefinition, MenuItem};
        let log = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let mut chain = NavChain::default();
        for name in ["MAIN", "CRM", "SALES", "CUST-LIST"] {
            chain.push(entry(name, false, &log));
        }
        let mut shell = Shell::default();
        shell.mount_root_menu(
            "MAIN",
            MenuDefinition {
                menu: vec![MenuItem::new_action("crm", "CRM")],
                hash: String::new(),
            },
        );
        let crm_menu = MenuDefinition {
            menu: vec![
                MenuItem::new_action("customers", "Customers"),
                MenuItem::new_action("leads", "Leads"),
            ],
            hash: String::new(),
        };
        let menu_of = {
            let crm = crm_menu.clone();
            move |form: &str| (form == "CRM").then(|| crm.clone())
        };

        let destroyed = breadcrumb_pop(&mut shell, &mut chain, 1, &menu_of);
        assert_eq!(
            destroyed,
            ["CUST-LIST", "SALES"],
            "AC10: deepest first, in order"
        );
        assert_eq!(chain.current().unwrap().form_object, "CRM");
        let (root, ctx_slot) = shell.mounted();
        assert_eq!(root.unwrap().form_object, "MAIN", "root slot untouched");
        assert_eq!(ctx_slot.unwrap().form_object, "CRM", "CRM's menu remounted");
        assert_eq!(ctx_slot.unwrap().def.menu.len(), 2);
        let events = log.borrow().clone();
        assert!(
            !events.contains(&"CRM:destroy".to_string()),
            "CRM stays resident: {events:?}"
        );
        // Popping to the MAIN form clears the contextual slot.
        let d2 = breadcrumb_pop(&mut shell, &mut chain, 0, &menu_of);
        assert_eq!(d2, ["CRM"]);
        assert!(shell.mounted().1.is_none(), "index 0 clears the slot");

        println!(
            "049 AC10 — pop to CRM destroyed [CUST-LIST, SALES] in order, \
             remounted CRM's 2-item menu (root untouched), CRM intact; pop to \
             MAIN then destroyed CRM and cleared the contextual slot"
        );
    }

    /// AC11 — a root-slot switch unwinds to the main form (chain AND parking
    /// lot destroyed) before the new subsystem is pushed.
    #[test]
    fn root_switch_unwinds_everything_first() {
        use cobolt_forms::menu::{MenuDefinition, MenuItem};
        let log = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let mut chain = NavChain::default();
        chain.push(entry("MAIN", false, &log));
        chain.push(entry("CRM", false, &log));
        chain.push(entry("CUST-LIST", true, &log));
        // Park CUST-LIST by replacing it with LEADS.
        chain.replace_top(entry("LEADS", false, &log));
        assert_eq!(chain.resident_count(), 4, "3 chained + 1 parked");

        let mut shell = Shell::default();
        let hr_menu = MenuDefinition {
            menu: vec![MenuItem::new_action("payroll", "Payroll")],
            hash: String::new(),
        };
        let destroyed = root_switch(
            &mut shell,
            &mut chain,
            entry("HR", false, &log),
            Some(hr_menu),
        );
        assert_eq!(
            destroyed,
            ["LEADS", "CRM", "CUST-LIST"],
            "AC11: chain (deepest first) then the parking lot"
        );
        assert_eq!(chain.segments().len(), 2, "MAIN › HR");
        assert_eq!(chain.current().unwrap().form_object, "HR");
        assert_eq!(shell.mounted().1.unwrap().form_object, "HR");
        let events = log.borrow().clone();
        assert!(
            !events.contains(&"MAIN:destroy".to_string()),
            "the main form is never destroyed by a switch: {events:?}"
        );

        println!(
            "049 AC11 — root switch destroyed [LEADS, CRM, CUST-LIST] (incl. \
             the parked form) before pushing HR; chain = MAIN › HR, HR's menu \
             mounted"
        );
    }

    /// AC12 — PreservePreviousForm: false ⇒ the outgoing sibling is destroyed
    /// and a return gets FRESH storage; true ⇒ parked, and the return revives
    /// the SAME resident (instant, storage intact).
    #[test]
    fn preserve_previous_form_parks_and_revives() {
        let log = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let mut chain = NavChain::default();
        chain.push(entry("MAIN", false, &log));
        chain.push(entry("CRM", false, &log));

        // preserve = FALSE: navigate away and back ⇒ destroy + a new entry.
        chain.push(entry("CUST-LIST", false, &log));
        let revived = chain.replace_top(entry("LEADS", false, &log));
        assert!(!revived);
        assert!(
            log.borrow().contains(&"CUST-LIST:destroy".to_string()),
            "false ⇒ destroyed on swap"
        );
        let revived_back = chain.replace_top(entry("CUST-LIST", false, &log));
        assert!(!revived_back, "the return is a FRESH entry (storage re-init)");

        // preserve = TRUE: park on swap, revive the SAME box on return.
        let before = log.borrow().len();
        let mut chain2 = NavChain::default();
        chain2.push(entry("MAIN", false, &log));
        chain2.push(entry("CRM", false, &log));
        chain2.push(entry("REPORTS", true, &log));
        chain2.replace_top(entry("LEADS", false, &log));
        assert_eq!(chain2.resident_count(), 4, "REPORTS parked, resident");
        let revived = chain2.replace_top(entry("REPORTS", false, &log));
        assert!(revived, "the parked REPORTS is revived, not rebuilt");
        assert_eq!(chain2.resident_count(), 3, "back in the chain");
        let events: Vec<String> = log.borrow()[before..].to_vec();
        assert!(
            !events.contains(&"REPORTS:destroy".to_string()),
            "true ⇒ never destroyed: {events:?}"
        );
        assert_eq!(
            events.iter().filter(|e| *e == "REPORTS:deactivate").count(),
            1,
            "one deactivate on the swap-out, none on revival"
        );

        println!(
            "049 AC12 — preserve=false: destroy on swap + fresh entry on \
             return; preserve=true: parked (resident_count 4), revived same \
             box (0 destroys, 1 deactivate), LEADS destroyed on the way back"
        );
    }

    /// AC13 — the event discipline through the standard ChannelResident: a
    /// resident swap-out fires onDeactivate and never onDestroy; a destroyed
    /// form fires onDestroy and no second onDeactivate.
    #[test]
    fn lifecycle_events_flow_through_the_channel_resident() {
        use std::sync::mpsc;
        let (ev_tx, ev_rx) = mpsc::channel();
        let entry = |name: &str, preserve: bool| NavEntry {
            form_object: name.to_string(),
            label: name.to_string(),
            preserve_on_replace: preserve,
            resident: Box::new(ChannelResident {
                form_object: name.to_string(),
                ev_tx: ev_tx.clone(),
            }),
        };

        let mut chain = NavChain::default();
        chain.push(entry("MAIN", false));
        chain.push(entry("CRM", false));
        // Sibling replace, outgoing NOT preserved: destroy, no deactivate.
        chain.push(entry("CUST-LIST", false));
        chain.replace_top(entry("LEADS", true));
        // Sibling replace, outgoing PRESERVED: deactivate only, parked.
        chain.replace_top(entry("REPORTS", false));
        assert_eq!(chain.resident_count(), 4, "LEADS parked, 3 in chain");
        // Breadcrumb pop to MAIN: REPORTS then CRM destroyed, deepest first.
        let destroyed = chain.pop_to(0);
        assert_eq!(destroyed, ["REPORTS", "CRM"]);

        let events: Vec<(String, String)> = ev_rx
            .try_iter()
            .map(|e| (e.ctrl_id, e.event_id))
            .collect();
        let of = |form: &str| -> Vec<&str> {
            events
                .iter()
                .filter(|(f, _)| f == form)
                .map(|(_, e)| e.as_str())
                .collect::<Vec<_>>()
        };
        assert_eq!(of("MAIN"), ["onDeactivate"], "ancestor: deactivate only");
        assert_eq!(
            of("CUST-LIST"),
            ["onDestroy"],
            "non-preserved sibling: destroy, no prior deactivate"
        );
        assert_eq!(
            of("LEADS"),
            ["onDeactivate"],
            "preserved sibling: deactivate only, never destroy"
        );
        assert_eq!(
            of("REPORTS"),
            ["onDestroy"],
            "breadcrumb pop: destroy without a second deactivate"
        );
        assert_eq!(of("CRM"), ["onDeactivate", "onDestroy"]);

        println!(
            "049 AC13 — event discipline over ChannelResident: ancestor \
             [onDeactivate]; non-preserved sibling [onDestroy]; preserved \
             sibling [onDeactivate] (parked, resident_count held at 4); \
             popped ancestor [onDeactivate, onDestroy]; {} events total",
            events.len()
        );
    }

    /// AC9 (chain half) — main → CRM → SALES → CUST-LIST: four breadcrumb
    /// segments in order, every push deactivates (never destroys) the
    /// previous top, and all four stay resident.
    #[test]
    fn chain_keeps_ancestors_resident_and_orders_the_breadcrumb() {
        let log = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let mut chain = NavChain::default();
        for name in ["MAIN", "CRM", "SALES", "CUST-LIST"] {
            chain.push(entry(name, false, &log));
        }
        let segs: Vec<String> = chain.segments().into_iter().map(|(f, _)| f).collect();
        assert_eq!(segs, ["MAIN", "CRM", "SALES", "CUST-LIST"], "R21 order");
        assert_eq!(chain.resident_count(), 4, "R20: all four resident");
        let events = log.borrow().clone();
        assert_eq!(
            events,
            [
                "MAIN:deactivate",
                "CRM:deactivate",
                "SALES:deactivate"
            ],
            "each push deactivates the previous top; NOTHING destroyed"
        );

        // The strip draws all four, and a click on the first segment resolves
        // to index 0. The click lands on the rect the LAYOUT reported — the
        // moment a test picks its own coordinate, it stops testing what the
        // operator's pointer actually hits.
        let ctx = egui::Context::default();
        let mut shell = Shell::default();
        shell.breadcrumb = chain
            .segments()
            .into_iter()
            .map(|(_, label)| label)
            .collect();
        frame(&mut shell, &ctx, raw(Vec2::new(1000.0, 700.0)));
        let l = shell.crumb_layout().expect("the strip laid out").clone();
        assert_eq!(l.segments.len(), 4, "R21: one segment per resident form");
        assert!(
            l.toggle.width() > 0.0 && l.toggle.max.x <= l.segments[0].min.x,
            "the sidebar toggle leads the chain"
        );

        let at = l.segments[0].center();
        let mut input = raw(Vec2::new(1000.0, 700.0));
        input.events.push(egui::Event::PointerMoved(at));
        input.events.push(egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        });
        input.events.push(egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        });
        frame(&mut shell, &ctx, input);
        assert_eq!(
            shell.take_breadcrumb_click(),
            Some(0),
            "clicking the first segment resolves to 0"
        );

        // The toggle at the head of the strip collapses the rail.
        let at = l.toggle.center();
        let mut input = raw(Vec2::new(1000.0, 700.0));
        input.events.push(egui::Event::PointerMoved(at));
        input.events.push(egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        });
        input.events.push(egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        });
        frame(&mut shell, &ctx, input);
        assert!(
            shell.take_toggle_request(),
            "the breadcrumb's icon is the sidebar's Open/Collapsed control"
        );

        println!(
            "049 AC9 (chain half) — 4 segments [MAIN › CRM › SALES › CUST-LIST], \
             3 deactivates, 0 destroys, resident_count=4; breadcrumb click on \
             segment 1 → index 0, and the {:.0}px toggle cell ahead of it \
             requests the rail. (The WORKING-STORAGE half of AC9 rides the \
             T27 spawn glue.)",
            l.toggle.width()
        );
    }

    /// 051 R18/R19 (AC10 shape) — the standalone menu actions submit real
    /// OpenForm requests with the SHELL as caller: Sync implicitly modal,
    /// Async never; a targetless action submits nothing; `event` items keep
    /// dispatching to the SideMenu handler beside them.
    #[test]
    fn standalone_menu_actions_submit_shell_parented_opens() {
        use crate::host::{FormHostConfig, NoHooks, Surface};
        use std::collections::HashMap;
        use std::sync::atomic::{AtomicBool, AtomicUsize};
        use std::sync::{mpsc, Arc};

        let form = cobolt_forms::Form::new("MAIN-FORM", "Main", 300, 200);
        let (ev_tx, ev_rx) = mpsc::channel();
        let (input_tx, input_rx) = mpsc::channel();
        let (_state_tx, state_rx) = mpsc::channel();
        let (_display_tx, display_rx) = mpsc::channel();
        let (_form_req_tx, form_req_rx) = mpsc::channel();
        let (closed_tx, _closed_rx) = mpsc::channel();
        let (host, _f) = crate::FormHost::new(FormHostConfig {
            form,
            flat: Vec::new(),
            state: HashMap::new(),
            ev_tx: ev_tx.clone(),
            input_tx: input_tx.clone(),
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

        // The app's request channel is the TEST's — every submitted open is
        // read back with its flags.
        let (test_tx, test_rx) = mpsc::channel();
        let mut app = ShellApp {
            shell: Shell::default(),
            chain: NavChain::default(),
            host,
            side_menu_ctrl: Some("SIDE-1".into()),
            state_path: None,
            input_tx,
            ev_tx,
            form_req_tx: test_tx,
        };
        let click = |action: &str| MenuClick {
            slot: MenuSlot::Root,
            item_id: format!("item-{action}"),
            action: Some(action.to_string()),
            preserve_previous_form: false,
        };
        app.shell.pending_clicks = vec![
            click("open-standalone-sync:REPORT"),
            click("open-standalone-async: MONITOR "),
            click("open-standalone-sync:"),
            click("event"),
        ];
        app.process_menu_clicks();

        let opens: Vec<(String, String, bool, bool)> = test_rx
            .try_iter()
            .map(|r| match r {
                cobolt_runtime::form_host::FormRequest::OpenForm {
                    caller,
                    form_id,
                    sync,
                    modal,
                    ..
                } => (caller, form_id, sync, modal),
                other => panic!("unexpected request: {other:?}"),
            })
            .collect();
        assert_eq!(
            opens,
            vec![
                ("W0".into(), "REPORT".into(), true, true),
                ("W0".into(), "MONITOR".into(), false, false),
            ],
            "sync ⇒ modal, async ⇒ modeless, caller is the shell, target trimmed, \
             empty target submits nothing"
        );
        // The `event` item still reached the SideMenu handler.
        let sel = input_rx.try_iter().find(|u: &cobolt_runtime::channels::StateUpdate| {
            u.prop == "SelectedItemId"
        });
        assert!(sel.is_some(), "event item wrote SelectedItemId");
        let ev: Vec<_> = ev_rx.try_iter().collect();
        assert!(
            ev.iter().any(|e| e.event_id == "onMenuItemClick"),
            "event item dispatched onMenuItemClick: {ev:?}"
        );
        println!(
            "051 standalone clicks — 4 clicks: 2 opens {opens:?}, 1 empty target \
             refused, 1 event dispatched"
        );
    }

    /// 051 R10/R11/R12 (AC1/AC3 shape) — the `open-form:` door: occupants
    /// swap with the chain, a preserving click parks the outgoing form (its
    /// instance — same handle — revives on return), a non-preserving click
    /// destroys it, and the breadcrumb truncates back to the main form.
    #[test]
    fn open_form_swaps_occupants_with_preserve_and_breadcrumb() {
        use crate::host::{FormHostConfig, FormSource, NoHooks, Surface};
        use std::collections::HashMap;
        use std::sync::atomic::{AtomicBool, AtomicUsize};
        use std::sync::{mpsc, Arc};

        fn program() -> cobolt_ast::program::Program {
            let src = "\
IDENTIFICATION DIVISION.\nPROGRAM-ID. CHILD.\nPROCEDURE DIVISION.\n    STOP RUN.\n";
            cobolt_parser::parse(cobolt_lexer::tokenize(src, cobolt_lexer::SourceFormat::Free))
                .program
                .expect("parses")
        }

        let form = cobolt_forms::Form::new("MAIN-FORM", "Main", 800, 600);
        let (ev_tx, ev_rx) = mpsc::channel();
        let (input_tx, _input_rx) = mpsc::channel();
        let (_state_tx, state_rx) = mpsc::channel();
        let (_display_tx, display_rx) = mpsc::channel();
        let (_form_req_tx, form_req_rx) = mpsc::channel();
        let (closed_tx, closed_rx) = mpsc::channel();
        let source: FormSource = Box::new(|id: &str| {
            let up = id.trim().to_ascii_uppercase();
            if up == "CRM" || up == "HR" {
                Ok((cobolt_forms::Form::new(up.as_str(), up.as_str(), 400, 300), program()))
            } else {
                Err(format!("no form named '{id}'"))
            }
        });
        let (host, _f) = crate::FormHost::new(FormHostConfig {
            form,
            flat: Vec::new(),
            state: HashMap::new(),
            ev_tx: ev_tx.clone(),
            input_tx: input_tx.clone(),
            state_rx,
            display_rx,
            pending: Arc::new(AtomicUsize::new(0)),
            finished: Arc::new(AtomicBool::new(false)),
            form_req_rx,
            closed_tx,
            form_req_tx: _form_req_tx.clone(),
            form_source: Some(source),
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

        let (test_tx, _test_rx) = mpsc::channel();
        let mut chain = NavChain::default();
        chain.push(NavEntry {
            form_object: "MAIN-FORM".into(),
            label: "Main".into(),
            preserve_on_replace: false,
            resident: Box::new(ChannelResident {
                form_object: "MAIN-FORM".into(),
                ev_tx: ev_tx.clone(),
            }),
        });
        let mut app = ShellApp {
            shell: Shell::default(),
            chain,
            host,
            side_menu_ctrl: None,
            state_path: None,
            input_tx,
            ev_tx,
            form_req_tx: test_tx,
        };
        let click = |action: &str, preserve: bool| MenuClick {
            slot: MenuSlot::Root,
            item_id: action.to_string(),
            action: Some(action.to_string()),
            preserve_previous_form: preserve,
        };

        // 1 — open CRM: it stacks on the main form and owns the pane.
        app.shell.pending_clicks = vec![click("open-form:CRM", false)];
        app.process_menu_clicks();
        assert_eq!(app.host.active_occupant_form(), Some("CRM"));
        assert_eq!(app.host.occupant_forms(), vec!["CRM".to_string()]);
        let crm_handle_1 = app.host.occupant_handle("CRM").expect("CRM registered");
        let segs: Vec<String> =
            app.chain.segments().into_iter().map(|(f, _)| f).collect();
        assert_eq!(segs, vec!["MAIN-FORM".to_string(), "CRM".to_string()]);

        // 2 — open HR, PRESERVING the outgoing CRM: it parks, instance kept.
        app.shell.pending_clicks = vec![click("open-form:HR", true)];
        app.process_menu_clicks();
        assert_eq!(app.host.active_occupant_form(), Some("HR"));
        assert_eq!(
            app.host.occupant_forms(),
            vec!["CRM".to_string(), "HR".to_string()],
            "the preserved CRM instance stays resident"
        );
        assert_eq!(app.chain.resident_count(), 3, "MAIN + HR + parked CRM");

        // 3 — back to CRM, NOT preserving HR: HR is destroyed; the parked
        // CRM revives — the very same instance (same supervisor handle).
        app.shell.pending_clicks = vec![click("open-form:CRM", false)];
        app.process_menu_clicks();
        assert_eq!(app.host.active_occupant_form(), Some("CRM"));
        assert_eq!(
            app.host.occupant_forms(),
            vec!["CRM".to_string()],
            "HR retired on a non-preserving swap"
        );
        assert_eq!(
            app.host.occupant_handle("CRM").as_deref(),
            Some(crm_handle_1.as_str()),
            "the preserved instance revived, not a rebuild"
        );
        // HR's release reached the fan-out (its windowHandlers NULL).
        let closed: Vec<String> = closed_rx.try_iter().collect();
        assert!(!closed.is_empty(), "HR's handle was released: {closed:?}");

        // 4 — breadcrumb back to the main form: CRM is destroyed, the pane
        // returns to the root, and the root (its lifecycle already fired)
        // gets a fresh onActivate.
        app.host.show_occupant(Some("CRM")); // ensure state
        app.chain.mark_top_preserve(false);
        {
            // The root's lifecycle pair already fired in a real run.
            app.host.root_lifecycle_sent_for_test();
        }
        let destroyed = app.chain.pop_to(0);
        app.host.retire_occupants(&destroyed);
        app.host.show_occupant(None);
        assert_eq!(app.host.active_occupant_form(), None);
        assert!(app.host.occupant_forms().is_empty(), "CRM destroyed");
        let root_events: Vec<String> = ev_rx.try_iter().map(|e| e.event_id).collect();
        assert!(
            root_events.iter().any(|e| e == "onActivate"),
            "the returning root re-activates: {root_events:?}"
        );

        println!(
            "051 occupant swap — CRM opened (chain MAIN›CRM), HR swap parked CRM \
             (3 resident), return revived the SAME instance ({crm_handle_1}), \
             breadcrumb-back destroyed it and re-activated the root"
        );
    }

    /// R21 — a loaded form's segment carries its designed **Title**, the same
    /// thing the main form's segment carries. The chain used to store the form
    /// OBJECT name for anything loaded through the `open-form:` door, so a
    /// shell whose root read "Main Menu" pointed at "inner-form1" — two
    /// vocabularies in one strip. A form with no title still falls back to its
    /// object name, which is all there is to show.
    #[test]
    fn a_loaded_forms_segment_is_its_title_not_its_object_name() {
        use crate::host::{FormHostConfig, FormSource, NoHooks, Surface};
        use std::collections::HashMap;
        use std::sync::atomic::{AtomicBool, AtomicUsize};
        use std::sync::{mpsc, Arc};

        fn program() -> cobolt_ast::program::Program {
            let src = "\
IDENTIFICATION DIVISION.\nPROGRAM-ID. CHILD.\nPROCEDURE DIVISION.\n    STOP RUN.\n";
            cobolt_parser::parse(cobolt_lexer::tokenize(src, cobolt_lexer::SourceFormat::Free))
                .program
                .expect("parses")
        }

        let form = cobolt_forms::Form::new("MAIN-FORM", "Main Menu", 800, 600);
        let (ev_tx, _ev_rx) = mpsc::channel();
        let (input_tx, _input_rx) = mpsc::channel();
        let (_state_tx, state_rx) = mpsc::channel();
        let (_display_tx, display_rx) = mpsc::channel();
        let (form_req_tx, form_req_rx) = mpsc::channel();
        let (closed_tx, _closed_rx) = mpsc::channel();
        // INNER-FORM1 is titled; PLAIN-FORM deliberately is not.
        let source: FormSource = Box::new(|id: &str| match id.trim().to_ascii_uppercase().as_str() {
            "INNER-FORM1" => Ok((
                cobolt_forms::Form::new("INNER-FORM1", "Customer Data", 400, 300),
                program(),
            )),
            "PLAIN-FORM" => Ok((
                cobolt_forms::Form::new("PLAIN-FORM", "", 400, 300),
                program(),
            )),
            other => Err(format!("no form named '{other}'")),
        });
        let (host, _f) = crate::FormHost::new(FormHostConfig {
            form,
            flat: Vec::new(),
            state: HashMap::new(),
            ev_tx: ev_tx.clone(),
            input_tx: input_tx.clone(),
            state_rx,
            display_rx,
            pending: Arc::new(AtomicUsize::new(0)),
            finished: Arc::new(AtomicBool::new(false)),
            form_req_rx,
            closed_tx,
            form_req_tx: form_req_tx.clone(),
            form_source: Some(source),
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

        let mut chain = NavChain::default();
        chain.push(NavEntry {
            form_object: "MAIN-FORM".into(),
            label: "Main Menu".into(),
            preserve_on_replace: false,
            resident: Box::new(ChannelResident {
                form_object: "MAIN-FORM".into(),
                ev_tx: ev_tx.clone(),
            }),
        });
        let mut app = ShellApp {
            shell: Shell::default(),
            chain,
            host,
            side_menu_ctrl: None,
            state_path: None,
            input_tx,
            ev_tx,
            form_req_tx,
        };
        let click = |action: &str| MenuClick {
            slot: MenuSlot::Root,
            item_id: action.to_string(),
            action: Some(action.to_string()),
            preserve_previous_form: false,
        };

        app.shell.pending_clicks = vec![click("open-form:INNER-FORM1")];
        app.process_menu_clicks();
        let labels: Vec<String> = app.chain.segments().into_iter().map(|(_, l)| l).collect();
        assert_eq!(
            labels,
            vec!["Main Menu".to_string(), "Customer Data".to_string()],
            "the title names the loaded form, not INNER-FORM1"
        );

        // A form with no title has only its object name to show.
        app.shell.pending_clicks = vec![click("open-form:PLAIN-FORM")];
        app.process_menu_clicks();
        let labels: Vec<String> = app.chain.segments().into_iter().map(|(_, l)| l).collect();
        assert_eq!(
            labels,
            vec!["Main Menu".to_string(), "PLAIN-FORM".to_string()],
            "no title ⇒ the object name is the honest label"
        );

        println!(
            "049 R21 breadcrumb labels — INNER-FORM1 (Title \"Customer Data\") \
             shows as \"Customer Data\"; PLAIN-FORM (no Title) shows as \
             \"PLAIN-FORM\"; 2/2 segments named by title-then-name"
        );
    }

    /// The breadcrumb's DETAIL level and the RESET it turns the form's own
    /// segment into.
    ///
    /// A form on the pane names what it is working on
    /// (`me::"SetBreadcrumbDetail"`); clicking its own name then asks the
    /// shell to start it over. The form has the last word: while its
    /// `PreventReset` guard is on, nothing is reset.
    #[test]
    fn a_detail_level_makes_the_form_name_a_reset_that_the_form_can_refuse() {
        use crate::host::{FormHostConfig, FormSource, NoHooks, Surface};
        use std::collections::HashMap;
        use std::sync::atomic::{AtomicBool, AtomicUsize};
        use std::sync::{mpsc, Arc};

        fn program() -> cobolt_ast::program::Program {
            let src = "\
IDENTIFICATION DIVISION.\nPROGRAM-ID. CHILD.\nPROCEDURE DIVISION.\n    STOP RUN.\n";
            cobolt_parser::parse(cobolt_lexer::tokenize(src, cobolt_lexer::SourceFormat::Free))
                .program
                .expect("parses")
        }

        let form = cobolt_forms::Form::new("MAIN-FORM", "Main Menu", 800, 600);
        let (ev_tx, _ev_rx) = mpsc::channel();
        let (input_tx, _input_rx) = mpsc::channel();
        let (_state_tx, state_rx) = mpsc::channel();
        let (_display_tx, display_rx) = mpsc::channel();
        let (form_req_tx, form_req_rx) = mpsc::channel();
        let (closed_tx, _closed_rx) = mpsc::channel();
        let source: FormSource = Box::new(|id: &str| {
            let up = id.trim().to_ascii_uppercase();
            match up.as_str() {
                "CUST" => Ok((
                    cobolt_forms::Form::new("CUST", "Customer Data", 400, 300),
                    program(),
                )),
                other => Err(format!("no form named '{other}'")),
            }
        });
        let (host, _f) = crate::FormHost::new(FormHostConfig {
            form,
            flat: Vec::new(),
            state: HashMap::new(),
            ev_tx: ev_tx.clone(),
            input_tx: input_tx.clone(),
            state_rx,
            display_rx,
            pending: Arc::new(AtomicUsize::new(0)),
            finished: Arc::new(AtomicBool::new(false)),
            form_req_rx,
            closed_tx,
            form_req_tx: form_req_tx.clone(),
            form_source: Some(source),
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

        let mut chain = NavChain::default();
        chain.push(NavEntry {
            form_object: "MAIN-FORM".into(),
            label: "Main Menu".into(),
            preserve_on_replace: false,
            resident: Box::new(ChannelResident {
                form_object: "MAIN-FORM".into(),
                ev_tx: ev_tx.clone(),
            }),
        });
        let mut app = ShellApp {
            shell: Shell::default(),
            chain,
            host,
            side_menu_ctrl: None,
            state_path: None,
            input_tx,
            ev_tx,
            form_req_tx,
        };
        app.shell.pending_clicks = vec![MenuClick {
            slot: MenuSlot::Root,
            item_id: "cust".into(),
            action: Some("open-form:CUST".into()),
            preserve_previous_form: false,
        }];
        app.process_menu_clicks();
        let first = app.host.occupant_handle("CUST").expect("CUST is on the pane");

        // The DISPLAYED form names what it is working on.
        app.apply_crumb_detail("CUST", Some("John Smith".into()));
        assert_eq!(app.shell.detail.as_deref(), Some("John Smith"));
        // A form that is NOT displayed has no name up there to hang it from.
        app.apply_crumb_detail("MAIN-FORM", Some("Nope".into()));
        assert_eq!(
            app.shell.detail.as_deref(),
            Some("John Smith"),
            "only the displayed form owns the crumb after its own name"
        );

        // Guarded: the click changes nothing at all.
        app.host.publish_prop_for_test("CUST", "PreventReset", "1");
        app.reset_displayed_form();
        assert_eq!(
            app.host.occupant_handle("CUST").as_deref(),
            Some(first.as_str()),
            "PreventReset: the very same instance is still on the pane"
        );
        assert_eq!(
            app.shell.detail.as_deref(),
            Some("John Smith"),
            "…and the crumb it set is still there"
        );

        // Guard lifted: the form starts over — a NEW instance, blank storage,
        // in the same place in the chain, with the crumb gone.
        app.host.publish_prop_for_test("CUST", "PreventReset", "0");
        app.reset_displayed_form();
        let second = app.host.occupant_handle("CUST").expect("CUST is back");
        assert_ne!(second, first, "a fresh instance replaced the old one");
        assert_eq!(app.shell.detail, None, "the crumb described data now gone");
        assert_eq!(
            app.chain
                .segments()
                .into_iter()
                .map(|(f, _)| f)
                .collect::<Vec<_>>(),
            vec!["MAIN-FORM".to_string(), "CUST".to_string()],
            "the chain is where it was — a reset is not a navigation"
        );
        assert_eq!(app.host.active_occupant_form(), Some("CUST"));
        assert_eq!(app.host.occupant_forms(), vec!["CUST".to_string()], "no leak");
        // Navigating away drops the crumb with the form that set it.
        app.apply_crumb_detail("CUST", Some("Jane Roe".into()));
        app.shell.pending_clicks = vec![MenuClick {
            slot: MenuSlot::Root,
            item_id: "home".into(),
            action: Some("home".into()),
            preserve_previous_form: false,
        }];
        app.process_menu_clicks();
        assert_eq!(app.shell.detail, None, "Home leaves no crumb behind");

        println!(
            "breadcrumb detail + reset — CUST set \"John Smith\" (a crumb from an \
             off-pane form was refused); PreventReset=1 → same instance {first} \
             and the crumb kept; PreventReset=0 → rebuilt as {second}, crumb \
             cleared, chain still MAIN-FORM›CUST; Home cleared the crumb"
        );
    }

    /// AC3 (mount half) — entering a subsystem replaces the contextual slot
    /// WHOLESALE while the root slot never changes; clicks carry the item's
    /// action and PreservePreviousForm flag.
    #[test]
    fn menu_slots_mount_root_once_and_swap_contextual_wholesale() {
        use cobolt_forms::menu::{MenuDefinition, MenuItem};

        let menu_of = |ids: &[(&str, &str)]| MenuDefinition {
            menu: ids
                .iter()
                .map(|(id, action)| MenuItem {
                    action: Some(action.to_string()),
                    ..MenuItem::new_action(*id, id.to_uppercase())
                })
                .collect(),
            hash: String::new(),
        };

        let mut shell = Shell::default();
        shell.mount_root_menu("MAIN-FORM", menu_of(&[("crm", "open-form:CRM"), ("hr", "open-form:HR")]));
        // R6 — a second root mount is ignored.
        shell.mount_root_menu("IMPOSTOR", menu_of(&[("x", "event")]));
        let (root, ctx_slot) = shell.mounted();
        assert_eq!(root.unwrap().form_object, "MAIN-FORM", "first mount wins");
        assert!(ctx_slot.is_none());

        // Enter CRM: the contextual slot is CRM's menu, root unchanged.
        shell.mount_contextual_menu(Some((
            "CRM".into(),
            menu_of(&[("customers", "open-form:CUST-LIST"), ("leads", "open-form:LEADS")]),
        )));
        let (root, ctx_slot) = shell.mounted();
        assert_eq!(root.unwrap().form_object, "MAIN-FORM");
        assert_eq!(ctx_slot.unwrap().form_object, "CRM");
        assert_eq!(ctx_slot.unwrap().def.menu.len(), 2);

        // Enter HR: WHOLESALE swap (R7) — no CRM item survives.
        shell.mount_contextual_menu(Some(("HR".into(), menu_of(&[("payroll", "open-form:PAYROLL")]))));
        let (root, ctx_slot) = shell.mounted();
        assert_eq!(root.unwrap().form_object, "MAIN-FORM", "root untouched");
        let hr = ctx_slot.unwrap();
        assert_eq!(hr.form_object, "HR");
        assert_eq!(hr.def.menu.len(), 1);
        assert_eq!(hr.def.menu[0].id, "payroll");

        // A click on a drawn item lands in take_menu_clicks with its action.
        let ctx = egui::Context::default();
        let size = Vec2::new(1000.0, 700.0);
        let mut frame_with = |shell: &mut Shell, input: egui::RawInput| {
            let mut full = ctx.run_ui(input, |root_ui| {
                shell.show(root_ui, |_ui| {}, |_ui| {});
            });
            full.textures_delta.clear();
        };
        frame_with(&mut shell, raw(size));
        // Click the item itself, wherever the pane put it — the pane's own
        // chrome (the Open/Collapsed toggle) sits above the items, so a fixed
        // coordinate would be a guess about layout rather than about menus.
        let click_at = shell
            .item_rect("crm")
            .expect("the root item was drawn")
            .center();
        let mut input = raw(size);
        input.events.push(egui::Event::PointerMoved(click_at));
        input.events.push(egui::Event::PointerButton {
            pos: click_at,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        });
        input.events.push(egui::Event::PointerButton {
            pos: click_at,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        });
        frame_with(&mut shell, input);
        let clicks = shell.take_menu_clicks();
        assert!(
            clicks
                .iter()
                .any(|c| c.slot == MenuSlot::Root
                    && c.item_id == "crm"
                    && c.action.as_deref() == Some("open-form:CRM")),
            "the click carries slot, id and action: {clicks:?}"
        );
        assert!(
            shell.take_menu_clicks().is_empty(),
            "clicks drain exactly once"
        );

        println!(
            "049 AC3 (mount half) — root mounted once (impostor ignored), \
             CRM→HR swapped the contextual slot wholesale with root untouched; \
             a real click on 'crm' drained as Root/open-form:CRM/preserve=false"
        );
    }

    /// R9/AC3 (state half) — the Collapsed state survives a save → load
    /// round trip (a host restart), and an absent file means Open.
    #[test]
    fn menu_pane_state_persists_across_restarts() {
        let dir = std::env::temp_dir().join("cobolt_test_shell_state_049");
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("APP-1").join("shell.toml");

        assert_eq!(load_collapsed_from(&path), None, "absent file ⇒ default Open");

        // "Collapse, restart, read back."
        save_collapsed_to(&path, true).expect("save");
        let mut restarted = Shell::default();
        if let Some(c) = load_collapsed_from(&path) {
            restarted.collapsed = c;
        }
        assert!(restarted.collapsed, "the restarted shell reads Collapsed");

        save_collapsed_to(&path, false).expect("save open");
        assert_eq!(load_collapsed_from(&path), Some(false));

        // The per-app path is namespaced and sanitised.
        let p = shell_state_path("My ERP/2.0").expect("home dir");
        let s = p.to_string_lossy();
        assert!(
            s.contains("cobolt") && s.contains("apps") && s.ends_with("shell.toml"),
            "path shape: {s}"
        );
        assert!(!s.contains("ERP/2"), "separators sanitised: {s}");

        let _ = std::fs::remove_dir_all(&dir);
        println!(
            "049 R9/AC3 — collapsed=true survived a simulated restart; \
             absent file ⇒ Open; app-name path sanitised ({})",
            p.file_name().unwrap().to_string_lossy()
        );
    }

    /// AC22 + AC6 (structural halves) — the MenuPane paints its OWN
    /// background, unchanged across loads of forms with different
    /// backgrounds (R39); and the breadcrumb strip is disjoint from the
    /// pane-backdrop rect, so no form background can touch it (R14).
    #[test]
    fn menu_background_is_immune_to_loaded_forms() {
        use crate::host::{FormHostConfig, NoHooks, Surface};
        use std::collections::HashMap;
        use std::sync::atomic::{AtomicBool, AtomicUsize};
        use std::sync::{mpsc, Arc};

        let mk_host = |bg: &str| {
            let mut form = cobolt_forms::Form::new("EMB", "Embedded", 300, 200);
            form.background_color = bg.into();
            let (ev_tx, _ev_rx) = mpsc::channel();
            let (input_tx, _input_rx) = mpsc::channel();
            let (_state_tx, state_rx) = mpsc::channel();
            let (_display_tx, display_rx) = mpsc::channel();
            let (_form_req_tx, form_req_rx) = mpsc::channel();
            let (closed_tx, _closed_rx) = mpsc::channel();
            let (host, _f) = crate::FormHost::new(FormHostConfig {
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
                form_req_tx: _form_req_tx.clone(),
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
            host
        };

        let ctx = egui::Context::default();
        let mut shell = Shell::default();
        shell.menu_background = Some(cobolt_forms::model::MenuPaneBackground {
            color: "#10305080".into(),
            ..Default::default()
        });
        let mut run = |shell: &mut Shell, host: &mut crate::FormHost| -> ShellLayout {
            let mut out = None;
            let mut full = ctx.run_ui(raw(Vec2::new(1000.0, 700.0)), |root_ui| {
                out = Some(shell.show_with_host(
                    root_ui,
                    |ui| {
                        ui.label("menu");
                    },
                    host,
                ));
            });
            full.textures_delta.clear();
            out.expect("shell ran")
        };

        // Three loads with clashing form backgrounds — the menu fill must not
        // move by a single channel.
        let mut fills = Vec::new();
        let mut layout = None;
        let mut backdrop_rect = None;
        for bg in ["FF0000FF", "00FF00FF", "10305080"] {
            let mut host = mk_host(bg);
            layout = Some(run(&mut shell, &mut host));
            backdrop_rect = host.pane_backdrop_rect();
            fills.push(shell.menu_fill().expect("menu painted its background"));
        }
        assert!(
            fills.windows(2).all(|w| w[0] == w[1]),
            "R39: the MenuPane fill must be identical across loads: {fills:?}"
        );

        // R14 — the breadcrumb is still the SHELL's paint, and a loaded form
        // can never repaint it. What changed is where it sits: the frame is
        // the top BAND of the content area (so the developer's own controls
        // can be placed over it), painted ON the pane backdrop rather than in
        // a panel above it — so the two now overlap by construction.
        let layout = layout.unwrap();
        let rect = backdrop_rect.expect("pane backdrop painted");
        assert!(
            rect.contains_rect(layout.breadcrumb_rect),
            "the breadcrumb frame is the content area's top band: {:?} vs pane {rect:?}",
            layout.breadcrumb_rect
        );
        assert!(
            rect.intersect(layout.menu_rect).width() <= 0.0
                || rect.intersect(layout.menu_rect).height() <= 0.0,
            "…nor the MenuPane: {rect:?} vs {:?}",
            layout.menu_rect
        );

        println!(
            "049 AC22/AC6 — menu fill {:?} constant across 3 form loads \
             (red/green/same-as-menu); form backdrop rect disjoint from the \
             MenuPane, and carrying the breadcrumb frame as its top band",
            fills[0]
        );
    }

    /// AC23 — a transparent form leaves the ContentPane region see-through
    /// (fill alpha 0) while the MenuPane and breadcrumb paint full alpha.
    /// (The `with_transparent` window creation itself is glue work — this is
    /// the paint contract that makes it safe.)
    #[test]
    fn transparent_form_reaches_the_desktop_through_the_pane_only() {
        use crate::host::{FormHostConfig, NoHooks, Surface};
        use std::collections::HashMap;
        use std::sync::atomic::{AtomicBool, AtomicUsize};
        use std::sync::{mpsc, Arc};

        let mut form = cobolt_forms::Form::new("EMB", "Embedded", 300, 200);
        // A form's see-through-ness is the Transparency PROPERTY (0-100),
        // never the colour's alpha byte (backdrop_color ignores it).
        form.background_color = "2E3138".into();
        form.transparency = 100;
        let (ev_tx, _ev_rx) = mpsc::channel();
        let (input_tx, _input_rx) = mpsc::channel();
        let (_state_tx, state_rx) = mpsc::channel();
        let (_display_tx, display_rx) = mpsc::channel();
        let (_form_req_tx, form_req_rx) = mpsc::channel();
        let (closed_tx, _closed_rx) = mpsc::channel();
        let (mut host, _f) = crate::FormHost::new(FormHostConfig {
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
            form_req_tx: _form_req_tx.clone(),
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

        let ctx = egui::Context::default();
        let mut shell = Shell::default(); // default chrome (no custom menu bg)
        let mut out = None;
        let mut full = ctx.run_ui(raw(Vec2::new(1000.0, 700.0)), |root_ui| {
            out = Some(shell.show_with_host(
                root_ui,
                |ui| {
                    ui.label("menu");
                },
                &mut host,
            ));
        });
        full.textures_delta.clear();
        let _ = out.expect("shell ran");

        let pane_fill = host.pane_backdrop_fill().expect("pane painted");
        assert_eq!(
            pane_fill.a(),
            0,
            "AC23: a fully transparent form leaves the pane region alpha 0: {pane_fill:?}"
        );
        let menu_fill = shell.menu_fill().expect("menu painted default chrome");
        assert_eq!(
            menu_fill.a(),
            255,
            "the MenuPane chrome stays opaque: {menu_fill:?}"
        );
        assert_eq!(CHROME_FILL.a(), 255, "the breadcrumb chrome is opaque");

        println!(
            "049 AC23 — transparent form: pane fill alpha 0 (see-through to \
             the desktop once the shell window is created transparent); \
             MenuPane fill alpha 255, breadcrumb chrome alpha 255"
        );
    }

    /// The rail composites its designed colour over the FORM's backdrop, the
    /// way the designer canvas does — it never paints that colour bare.
    ///
    /// The operator photographed the failure: a sidebar designed
    /// `#F6F6F639` (white at 22 %) read as a dark navy rail on the canvas,
    /// because 22 % white over the form's navy is a slightly lighter navy —
    /// and shipped as a WHITE rail in the running shell, because the same
    /// colour painted into a transparent window (R43) composites over the
    /// desktop instead.
    #[test]
    fn a_translucent_rail_colour_composites_over_the_form_backdrop() {
        let navy = cobolt_forms::render::backdrop_color("00000000", 0);
        let mut side = cobolt_forms::Control::new(
            "SideMenu-1",
            cobolt_forms::ControlType::SideMenu,
            0,
            0,
        );
        side.set_prop("BackgroundColor", "#F6F6F639");

        let ctx = egui::Context::default();
        let mut shell = Shell::default();
        shell.side_ctrl = Some(side);
        shell.form_backdrop = Some(navy);
        let _ = frame(&mut shell, &ctx, raw(Vec2::new(1000.0, 700.0)));
        let fill = shell.menu_fill().expect("the rail painted");

        assert_eq!(
            fill.a(),
            255,
            "R43: the rail is opaque chrome, whatever alpha it was designed \
             with: {fill:?}"
        );
        // 22 % white over the navy: lighter than the form, nowhere near white.
        for (ch, base) in [
            (fill.r(), navy.r()),
            (fill.g(), navy.g()),
            (fill.b(), navy.b()),
        ] {
            assert!(ch > base, "the rail lifts off the backdrop: {fill:?}");
            assert!(
                ch < 128,
                "the rail must NOT wash out to white — that is the shipped \
                 bug: {fill:?}"
            );
        }

        // With no form backdrop to stand on, the chrome constant still is one.
        let mut bare = Shell::default();
        bare.side_ctrl = shell.side_ctrl.clone();
        let _ = frame(&mut bare, &ctx, raw(Vec2::new(1000.0, 700.0)));
        assert_eq!(
            bare.menu_fill().expect("painted").a(),
            255,
            "R43 holds without a form backdrop too"
        );

        println!(
            "049 — sidebar #F6F6F639 (alpha 57) over form backdrop {:?} → \
             rail {:?}, opaque and still dark; painted bare it was white",
            navy, fill
        );
    }

    /// A rail toggle moves the WINDOW by the rail's width, so the ContentPane
    /// keeps its own and a round trip lands back where it started.
    #[test]
    fn toggling_the_rail_resizes_the_window_not_the_content_pane() {
        let (open_w, collapsed_w) = (200.0_f32, MENU_PANE_COLLAPSED_WIDTH);
        let delta = open_w - collapsed_w;
        let start = 1100.0_f32;

        // Collapsed → Open: the window grows by exactly the rail.
        let opened = shell_width_for_pane(start, open_w, collapsed_w, true);
        assert_eq!(opened, start + delta);
        // …and the pane the developer designed against is unchanged.
        assert_eq!(
            opened - open_w,
            start - collapsed_w,
            "the ContentPane keeps its width across the toggle"
        );

        // Open → Collapsed: the window gives the width back, exactly.
        let closed = shell_width_for_pane(opened, open_w, collapsed_w, false);
        assert_eq!(closed, start, "a round trip returns the original size");

        // A rail no wider than the collapsed strip moves nothing.
        assert_eq!(
            shell_width_for_pane(start, collapsed_w, collapsed_w, true),
            start
        );
        // Collapsing never shrinks the window away on a narrow screen.
        assert_eq!(
            shell_width_for_pane(500.0, 900.0, 48.0, false),
            MIN_SHELL_WIDTH,
            "the self-resize is floored"
        );

        println!(
            "049 — rail {open_w:.0}px vs {collapsed_w:.0}px rail: window \
             {start:.0} → {opened:.0} on open → {closed:.0} on collapse; \
             ContentPane {:.0}px throughout",
            start - collapsed_w
        );
    }

    /// A shell window opens at the size the form was DESIGNED at, not a fixed
    /// 1100x700 — which clipped anything wider on its very first frame.
    #[test]
    fn a_shell_window_opens_at_the_designed_size() {
        let (form_w, form_h) = (960.0_f32, 744.0_f32);
        let rail = 200.0_f32;

        // Rail open: the form's own width IS the window width, because the
        // designed width already spans the rail and the content beside it.
        let open = shell_window_size(form_w, form_h, rail, rail, 0.0);
        assert_eq!(open.x, form_w, "the designed width, exactly");
        assert_eq!(
            open.y, form_h,
            "the designed height, exactly: a FullHeight rail's breadcrumb \
             OVERLAYS the form's own top band and costs the window nothing"
        );
        // FullHeight off puts the strip in a panel of its own above the whole
        // window, and that band is not the form's — so the window pays for it.
        assert_eq!(
            shell_window_size(form_w, form_h, rail, rail, BREADCRUMB_HEIGHT).y,
            form_h + BREADCRUMB_HEIGHT
        );
        // The ContentPane is then exactly the content the developer drew.
        assert_eq!(open.x - rail, form_w - rail);

        // Opening collapsed gives the difference back, so the content pane is
        // the same width in both states — the rule the toggle already follows.
        let collapsed =
            shell_window_size(form_w, form_h, rail, MENU_PANE_COLLAPSED_WIDTH, 0.0);
        assert_eq!(collapsed.x, form_w - rail + MENU_PANE_COLLAPSED_WIDTH);
        assert_eq!(
            collapsed.x - MENU_PANE_COLLAPSED_WIDTH,
            open.x - rail,
            "the ContentPane opens the same size whichever state the rail is in"
        );

        // A tiny form still gets a usable window.
        assert_eq!(shell_window_size(100.0, 80.0, 60.0, 60.0, 0.0).x, MIN_SHELL_WIDTH);

        println!(
            "049 — a {form_w:.0}x{form_h:.0} form with a {rail:.0}px rail opens \
             {:.0}x{:.0} (was a fixed 1100x700); collapsed it opens {:.0} wide, \
             same {:.0}px ContentPane either way",
            open.x,
            open.y,
            collapsed.x,
            open.x - rail
        );
    }

    /// R8 — Collapsed is the narrow rail; the ContentPane gains exactly the
    /// difference.
    #[test]
    fn collapse_swaps_to_the_rail_width() {
        let ctx = egui::Context::default();
        let mut shell = Shell::default();
        let open = frame(&mut shell, &ctx, raw(Vec2::new(1000.0, 700.0)));
        shell.collapsed = true;
        let collapsed = frame(&mut shell, &ctx, raw(Vec2::new(1000.0, 700.0)));
        assert!((open.menu_rect.width() - MENU_PANE_OPEN_WIDTH).abs() < 1.0);
        assert!((collapsed.menu_rect.width() - MENU_PANE_COLLAPSED_WIDTH).abs() < 1.0);
        let gained = collapsed.content_rect.width() - open.content_rect.width();
        let expect = MENU_PANE_OPEN_WIDTH - MENU_PANE_COLLAPSED_WIDTH;
        assert!(
            (gained - expect).abs() < 1.0,
            "the ContentPane gains the rail difference: {gained} vs {expect}"
        );
        println!(
            "049 R8 — Open {:.0}px → Collapsed {:.0}px; ContentPane gained \
             {:.0}px (exactly the difference)",
            open.menu_rect.width(),
            collapsed.menu_rect.width(),
            gained
        );
    }
}

#[cfg(test)]
mod shell_event_tests {
    use super::*;
    use crate::host::{FormHostConfig, NoHooks, Surface};
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicBool, AtomicUsize};
    use std::sync::{mpsc, Arc};

    fn raw(size: Vec2) -> egui::RawInput {
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, size)),
            ..Default::default()
        }
    }

    /// One click on a control inside a SHELL-hosted form must fire ONE event.
    ///
    /// The operator's `sidebar-form.cfrm` is a main form with a **SideMenu**, so
    /// it runs on the shell path — the form is the ContentPane occupant, not a
    /// plain window. Its `Switch-1` has exactly one `onClick` binding, and a
    /// single click ran the handler twice (2026-08-21). The equivalent scene on
    /// the plain window path fires once, so the shell is where this has to be
    /// measured.
    #[test]
    fn a_click_in_a_shell_hosted_form_fires_one_event() {
        let mut form = cobolt_forms::Form::new("SIDEBAR-FORM", "Main Menu", 900, 600);
        form.main_form = true;
        let mut rail =
            cobolt_forms::Control::new("SIDE-1", cobolt_forms::ControlType::SideMenu, 0, 0);
        rail.rect = cobolt_forms::model::Rect::new(0, 0, 200, 600);
        let mut panel =
            cobolt_forms::Control::new("Panel-8", cobolt_forms::ControlType::Panel, 0, 0);
        panel.rect = cobolt_forms::model::Rect::new(0, 0, 300, 200);
        let mut sw =
            cobolt_forms::Control::new("Switch-1", cobolt_forms::ControlType::Switch, 10, 10);
        sw.rect = cobolt_forms::model::Rect::new(10, 10, 48, 28);
        sw.parent = Some("Panel-8".into());
        // BOUND, like the operator's Switch-1 — `onClick` is emitted only for a
        // control that binds a handler.
        sw.ensure_event("onClick");
        let flat = vec![rail, panel, sw];

        let (ev_tx, ev_rx) = mpsc::channel();
        let (input_tx, _input_rx) = mpsc::channel();
        let (_state_tx, state_rx) = mpsc::channel();
        let (_display_tx, display_rx) = mpsc::channel();
        let (_form_req_tx, form_req_rx) = mpsc::channel();
        let (closed_tx, _closed_rx) = mpsc::channel();
        let (mut host, _f) = crate::FormHost::new(FormHostConfig {
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

        let ctx = egui::Context::default();
        let mut shell = Shell::default();
        let size = Vec2::new(1000.0, 700.0);
        let mut run = |ctx: &egui::Context,
                       shell: &mut Shell,
                       host: &mut crate::FormHost,
                       input: egui::RawInput| {
            let mut full = ctx.run_ui(input, |root_ui| {
                let _ = shell.show_with_host(root_ui, |_ui| {}, host);
            });
            full.textures_delta.clear();
        };

        // Past the host's 450 ms arming window.
        run(&ctx, &mut shell, &mut host, raw(size));
        std::thread::sleep(std::time::Duration::from_millis(500));
        for _ in 0..2 {
            run(&ctx, &mut shell, &mut host, raw(size));
        }
        while ev_rx.try_recv().is_ok() {}

        // Where the switch landed: the form anchors at the pane's top-left, so
        // the designed offset plus the pane origin is the control's centre.
        let pane = host.pane_backdrop_rect().expect("the pane was painted");
        let at = pane.min + Vec2::new(10.0 + 24.0, 10.0 + 14.0);

        let mut down = raw(size);
        down.events = vec![
            egui::Event::PointerMoved(at),
            egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            },
        ];
        run(&ctx, &mut shell, &mut host, down);
        let mut up = raw(size);
        up.events = vec![egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        }];
        run(&ctx, &mut shell, &mut host, up);
        for _ in 0..3 {
            run(&ctx, &mut shell, &mut host, raw(size));
        }

        let evs: Vec<(String, String)> = ev_rx
            .try_iter()
            .map(|e| (e.ctrl_id, e.event_id))
            .collect();
        let clicks: Vec<_> = evs
            .iter()
            .filter(|(c, e)| c == "Switch-1" && e.eq_ignore_ascii_case("onClick"))
            .collect();
        assert_eq!(
            clicks.len(),
            1,
            "one click on a shell-hosted control must fire one onClick; got {evs:?}"
        );
    }
}
