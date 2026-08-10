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
pub const CHROME_FILL: egui::Color32 = egui::Color32::from_rgb(0x2E, 0x31, 0x38);

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

/// R21/R22 — draw the chain as breadcrumb segments (chain order, `›`
/// separated) into the breadcrumb strip's `Ui`; returns the clicked segment
/// index, which the navigation layer feeds to [`NavChain::pop_to`].
pub fn draw_breadcrumb(ui: &mut Ui, chain: &NavChain) -> Option<usize> {
    let mut clicked = None;
    ui.horizontal(|ui| {
        for (i, (_, label)) in chain.segments().iter().enumerate() {
            if i > 0 {
                ui.label("›");
            }
            if ui.link(label).clicked() {
                clicked = Some(i);
            }
        }
    });
    clicked
}

/// The breadcrumb strip — the same chrome in both layout orders, so
/// FullHeight changes only WHEN it is created, never what it is.
fn show_breadcrumb(root_ui: &mut Ui, breadcrumb: Option<impl FnOnce(&mut Ui)>) -> Rect {
    let Some(breadcrumb) = breadcrumb else {
        return Rect::NOTHING;
    };
    egui::Panel::top("shell-breadcrumb")
        .resizable(false)
        .exact_size(BREADCRUMB_HEIGHT)
        .show(root_ui, |ui| {
            // R43 — explicit opaque chrome (see CHROME_FILL).
            ui.painter().rect_filled(ui.max_rect(), 0.0, CHROME_FILL);
            breadcrumb(ui);
        })
        .response
        .rect
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
    /// What the MenuPane actually painted last frame (tests/parity).
    last_menu_fill: Option<egui::Color32>,
    /// Where the Open/Collapsed toggle landed last frame (tests/parity).
    last_toggle_rect: Option<Rect>,
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
            last_menu_fill: None,
            last_toggle_rect: None,
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
        let Some(mp) = &self.menu_background else {
            // R43 — the default chrome is painted EXPLICITLY: in a
            // transparent shell window, skipping the paint would leave the
            // MenuPane see-through.
            ui.painter().rect_filled(ui.max_rect(), 0.0, CHROME_FILL);
            self.last_menu_fill = Some(CHROME_FILL);
            return;
        };
        let backdrop = cobolt_forms::render::Backdrop {
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
        self.last_menu_fill = Some(painted.bg);
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

    /// Drain a click on the pane's Open/Collapsed toggle. The caller flips
    /// [`Self::collapsed`] and persists it (R9) — the shell does not persist
    /// on its own, so tests can drive the toggle without touching disk.
    pub fn take_toggle_request(&mut self) -> bool {
        std::mem::take(&mut self.toggle_requested)
    }

    /// Draw the pane's own Open/Collapsed toggle, at the top of the MenuPane.
    /// Unconditional by design: an application whose menu is still empty must
    /// stay collapsible, so this is drawn before — and independently of — the
    /// mounted slots.
    /// The glyph carries the whole affordance: `cobolt-form-host` has no
    /// access to the IDE's `Tr` table, so any English tooltip here would be
    /// untranslatable chrome in six-language software.
    fn draw_pane_toggle(&mut self, ui: &mut Ui) {
        let resp = ui.button(MENU_PANE_TOGGLE);
        self.last_toggle_rect = Some(resp.rect);
        if resp.clicked() {
            self.toggle_requested = true;
        }
        ui.separator();
    }

    /// Where the pane's toggle landed last frame (tests drive it from here).
    pub fn toggle_rect(&self) -> Option<Rect> {
        self.last_toggle_rect
    }

    /// Draw both mounted slots into the MenuPane's scroll `Ui` and collect
    /// clicks. Open: labels (submenu items indented). Collapsed: the rail
    /// keeps the ROOT items reachable as single-glyph buttons (R8).
    fn draw_mounted_menus(&mut self, ui: &mut Ui) {
        let collapsed = self.collapsed;
        let icon_effect = self.icon_effect.clone();
        let mut clicks = Vec::new();
        let mut rects: Vec<(String, Rect)> = Vec::new();
        // One item row: icon (styled by the SideMenu's IconEffect) + label.
        // Collapsed rows are icon-only — an item with no icon falls back to
        // its first letter, so every item stays reachable on the rail.
        let item_row = |ui: &mut Ui,
                        icon: &Option<String>,
                        label: &str,
                        enabled: bool|
         -> egui::Response {
            let icon_sz = 18.0;
            let tint = if enabled {
                ui.visuals().text_color()
            } else {
                ui.visuals().weak_text_color()
            };
            let style = cobolt_forms::icons::icon_style_for_effect(&icon_effect, tint);
            if collapsed {
                match icon {
                    Some(name) => {
                        let (rect, resp) = ui.allocate_exact_size(
                            Vec2::splat(icon_sz + 6.0),
                            egui::Sense::click(),
                        );
                        if resp.hovered() && enabled {
                            ui.painter().rect_filled(
                                rect,
                                4.0,
                                ui.visuals().widgets.hovered.weak_bg_fill,
                            );
                        }
                        cobolt_forms::icons::draw_menu_icon_styled(
                            ui.painter(),
                            Rect::from_center_size(rect.center(), Vec2::splat(icon_sz)),
                            name,
                            &style,
                        );
                        resp
                    }
                    None => {
                        let initial =
                            label.chars().next().map(String::from).unwrap_or_default();
                        ui.add_enabled(enabled, egui::Button::new(initial))
                    }
                }
            } else {
                ui.horizontal(|ui| {
                    if let Some(name) = icon {
                        let (rect, _) = ui
                            .allocate_exact_size(Vec2::splat(icon_sz), egui::Sense::hover());
                        cobolt_forms::icons::draw_menu_icon_styled(
                            ui.painter(),
                            rect,
                            name,
                            &style,
                        );
                    }
                    ui.add_enabled(enabled, egui::Button::new(label))
                })
                .inner
            }
        };
        let mut draw_slot = |slot: MenuSlot, m: &MountedMenu, ui: &mut Ui| {
            for item in &m.def.menu {
                if item.item_type == cobolt_forms::menu::MenuItemType::Separator {
                    ui.separator();
                    continue;
                }
                let resp = item_row(ui, &item.icon, &item.label, item.enabled);
                rects.push((item.id.clone(), resp.rect));
                if resp.clicked() {
                    clicks.push(MenuClick {
                        slot,
                        item_id: item.id.clone(),
                        action: item.action.clone(),
                        preserve_previous_form: item.preserve_previous_form,
                    });
                }
                if !collapsed {
                    for sub in &item.items {
                        if sub.item_type == cobolt_forms::menu::MenuItemType::Separator {
                            continue;
                        }
                        ui.indent(&sub.id, |ui| {
                            let r = item_row(ui, &sub.icon, &sub.label, sub.enabled);
                            rects.push((sub.id.clone(), r.rect));
                            if r.clicked() {
                                clicks.push(MenuClick {
                                    slot,
                                    item_id: sub.id.clone(),
                                    action: sub.action.clone(),
                                    preserve_previous_form: sub.preserve_previous_form,
                                });
                            }
                        });
                    }
                }
            }
        };
        if let Some(root) = self.root_menu.clone() {
            draw_slot(MenuSlot::Root, &root, ui);
        }
        if let Some(ctx_menu) = self.contextual_menu.clone() {
            if !collapsed {
                ui.separator();
                draw_slot(MenuSlot::Contextual, &ctx_menu, ui);
            }
        }
        self.pending_clicks.extend(clicks);
        self.last_item_rects = rects;
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
        breadcrumb: impl FnOnce(&mut Ui),
        content: impl FnOnce(&mut Ui),
    ) -> ShellLayout {
        let mut menu_scroll = Vec2::ZERO;
        // 049 — panel ORDER is what decides who owns the corner: whichever is
        // created first spans its full axis. FullHeight therefore reads as
        // "the MenuPane goes in first".
        let mut breadcrumb_rect = Rect::NOTHING;
        let mut breadcrumb = Some(breadcrumb);
        if !self.full_height {
            breadcrumb_rect = show_breadcrumb(root_ui, breadcrumb.take());
        }
        let panel = egui::Panel::left("shell-menu-pane")
            .resizable(false)
            .exact_size(self.menu_pane_width())
            .show(root_ui, |ui| {
                // R39 — the shell's own chrome paint, before any content.
                self.paint_menu_background(ui);
                // R37 — the MenuPane's own scroll area, id distinct from the
                // ContentPane's by construction.
                let out = egui::ScrollArea::vertical()
                    .id_salt("shell-menu-scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        // The toggle first — it must survive an empty menu.
                        self.draw_pane_toggle(ui);
                        // R6/R7 — the mounted slots draw next; the caller's
                        // closure may add extra chrome below them.
                        self.draw_mounted_menus(ui);
                        menu(ui)
                    });
                menu_scroll = out.state.offset;
            });
        let menu_rect = panel.response.rect;

        if breadcrumb.is_some() {
            breadcrumb_rect = show_breadcrumb(root_ui, breadcrumb);
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
        breadcrumb: impl FnOnce(&mut Ui),
        host: &mut crate::FormHost,
    ) -> ShellLayout {
        let mut menu_scroll = Vec2::ZERO;
        // See `show` — panel order is what makes FullHeight true or false.
        let mut breadcrumb_rect = Rect::NOTHING;
        let mut breadcrumb = Some(breadcrumb);
        if !self.full_height {
            breadcrumb_rect = show_breadcrumb(root_ui, breadcrumb.take());
        }
        let panel = egui::Panel::left("shell-menu-pane")
            .resizable(false)
            .exact_size(self.menu_pane_width())
            .show(root_ui, |ui| {
                // R39 — the shell's own chrome paint, before any content.
                self.paint_menu_background(ui);
                let out = egui::ScrollArea::vertical()
                    .id_salt("shell-menu-scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        // The toggle first — it must survive an empty menu.
                        self.draw_pane_toggle(ui);
                        // R6/R7 — the mounted slots draw next.
                        self.draw_mounted_menus(ui);
                        menu(ui)
                    });
                menu_scroll = out.state.offset;
            });
        let menu_rect = panel.response.rect;

        if breadcrumb.is_some() {
            breadcrumb_rect = show_breadcrumb(root_ui, breadcrumb);
        }

        // The remaining space IS the ContentPane; the host's own
        // CentralPanel + ScrollArea consume it.
        let content_rect = root_ui.available_rect_before_wrap();
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
    let viewport = egui::ViewportBuilder::default()
        .with_title(&title)
        .with_inner_size([1100.0, 700.0])
        .with_resizable(true)
        // R43 — the shell window carries alpha; the chrome paints itself.
        .with_transparent(true);
    let native_options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    let app = ShellApp {
        shell,
        chain,
        host,
        side_menu_ctrl,
        state_path,
        input_tx,
        ev_tx,
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
}

impl ShellApp {
    fn persist_collapsed(&self) {
        if let Some(p) = &self.state_path {
            let _ = save_collapsed_to(p, self.shell.collapsed);
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
        let mut crumb_click = None;
        let chain_ref = &self.chain;
        let shell = &mut self.shell;
        let host = &mut self.host;
        shell.show_with_host(
            root_ui,
            |_ui| {},
            |ui| {
                // The pane owns its own ☰ (drawn on the MenuPane itself, so it
                // works with an empty menu); the breadcrumb is just the chain.
                if let Some(i) = draw_breadcrumb(ui, chain_ref) {
                    crumb_click = Some(i);
                }
            },
            host,
        );
        if self.shell.take_toggle_request() {
            self.shell.collapsed = !self.shell.collapsed;
            self.persist_collapsed();
        }
        // R44 — COBOL drove the pane through the supervisor.
        if let Some(collapsed) = self.host.take_menu_pane_request() {
            self.shell.collapsed = collapsed;
            self.persist_collapsed();
        }
        // Menu activations (root slot only until the multi-form host lands).
        for click in self.shell.take_menu_clicks() {
            match click.action.as_deref() {
                Some(a) if a.starts_with("open-form:") => {
                    // Honest limit: a second form needs its own interpreter +
                    // program, the same open work as 037 T16.
                    eprintln!(
                        "shell: menu item '{}' wants {a}, but hosting a second \
                         form awaits the multi-form host (037 T16 / 049 tasks \
                         note)",
                        click.item_id
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
        // A breadcrumb click on the sole entry is a no-op today.
        let _ = crumb_click;
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
                    ui.label("crumbs");
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
            fx_entrance: cobolt_forms::window_fx::FxSpec::parse("fade:2000:linear"),
            fx_exit: cobolt_forms::window_fx::FxSpec::default(),
            fx_restore: false,
            theme_pack: None,
            surface_style: cobolt_forms::paint::SurfaceStyle::LiquidGlass,
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
                    |ui| {
                        ui.label("crumbs");
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
                fx_entrance: cobolt_forms::window_fx::FxSpec::default(),
                fx_exit: cobolt_forms::window_fx::FxSpec::default(),
                fx_restore: false,
                theme_pack: None,
                surface_style: cobolt_forms::paint::SurfaceStyle::LiquidGlass,
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
                    |ui| {
                        ui.label("crumbs");
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
                fx_entrance: cobolt_forms::window_fx::FxSpec::parse("fade:2000:linear"),
                fx_exit: cobolt_forms::window_fx::FxSpec::default(),
                fx_restore: false,
                theme_pack: None,
                surface_style: cobolt_forms::paint::SurfaceStyle::LiquidGlass,
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
            shell.show_with_host(root_ui, |_ui| {}, |_ui| {}, &mut pane);
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

        // The breadcrumb draws all four and a click on the first segment
        // resolves to index 0.
        let ctx = egui::Context::default();
        let mut clicked = None;
        let mut frame = |input: egui::RawInput| {
            let mut full = ctx.run_ui(input, |root_ui| {
                egui::Panel::top("bc-test")
                    .resizable(false)
                    .exact_size(28.0)
                    .show(root_ui, |ui| {
                        if let Some(i) = draw_breadcrumb(ui, &chain) {
                            clicked = Some(i);
                        }
                    });
            });
            full.textures_delta.clear();
        };
        frame(raw(Vec2::new(1000.0, 700.0)));
        let at = egui::pos2(18.0, 14.0); // inside the first segment
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
        frame(input);
        assert_eq!(clicked, Some(0), "clicking the first segment resolves to 0");

        println!(
            "049 AC9 (chain half) — 4 segments [MAIN › CRM › SALES › CUST-LIST], \
             3 deactivates, 0 destroys, resident_count=4; breadcrumb click on \
             segment 1 → index 0. (The WORKING-STORAGE half of AC9 rides the \
             T27 spawn glue.)"
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
                shell.show(root_ui, |_ui| {}, |_ui| {}, |_ui| {});
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
                fx_entrance: cobolt_forms::window_fx::FxSpec::default(),
                fx_exit: cobolt_forms::window_fx::FxSpec::default(),
                fx_restore: false,
                theme_pack: None,
                surface_style: cobolt_forms::paint::SurfaceStyle::LiquidGlass,
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
                    |ui| {
                        ui.label("crumbs");
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

        // R14 — the pane backdrop can never reach the breadcrumb strip.
        let layout = layout.unwrap();
        let rect = backdrop_rect.expect("pane backdrop painted");
        assert!(
            rect.intersect(layout.breadcrumb_rect).height() <= 0.0
                || rect.intersect(layout.breadcrumb_rect).width() <= 0.0,
            "the form backdrop must not overlap the breadcrumb: {rect:?} vs {:?}",
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
             breadcrumb strip and the MenuPane",
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
            fx_entrance: cobolt_forms::window_fx::FxSpec::default(),
            fx_exit: cobolt_forms::window_fx::FxSpec::default(),
            fx_restore: false,
            theme_pack: None,
            surface_style: cobolt_forms::paint::SurfaceStyle::LiquidGlass,
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
                |ui| {
                    ui.label("crumbs");
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
