// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The ONE sidebar renderer (spec 049).
//!
//! A SideMenu is drawn on four surfaces — the designer canvas, the form
//! preview, Run Form, and the running shell's MenuPane. Before this module
//! each of those painted its own rail, which is why icons appeared on one
//! surface and not another and why the preview's ☰ was inert. Layout,
//! painting and hit-testing all live here now; a caller supplies the rect and
//! the state, gets back the row rectangles, and decides what a click means.
//!
//! Anatomy, top to bottom:
//!
//! ```text
//!   ┌──────────────┐        ┌────┐
//!   │ ▲  AppTitle  │        │ ▲  │   header  — logo + title (title hidden
//!   ├──────────────┤        ├────┤             on the collapsed rail)
//!   │ HOME         │        │ ⋯  │   section — a Separator WITH a label;
//!   │ ▣ Modern  New│        │ ▣  │             collapsed, it becomes the
//!   │ ▤ Analytical │        │ ▤  │             ellipsis group divider
//!   │ APPS         │        │ ⋯  │   row     — icon + label + badge/chevron
//!   │ ▧ Chat       │        │ ▧  │
//!   ├──────────────┤        ├────┤
//!   │ ☻ Name  Role │        │ ☻  │   profile — anchored to the bottom
//!   └──────────────┘        └────┘
//! ```
//!
//! **Colour comes from the control**, never from constants: the SideMenu's
//! `BackgroundColor`, `ForegroundColor`, `HighlightBgColor`, `SelectedBgColor`
//! and `SelectedFgColor` properties carry the application's theme, so a rail
//! restyles with the app instead of being locked to one palette.

use crate::menu::{BadgeStyle, MenuItem, MenuItemType};
use crate::model::Control;
use egui::{Color32, FontId, Pos2, Rect, Stroke, Vec2};

/// Default rail widths, in points.
pub const OPEN_WIDTH: f32 = 240.0;
/// The collapsed icon rail's width. **48 is the reference look** — the running
/// shell's MenuPane (the operator's "only correct" collapsed rail,
/// 2026-08-23); the designer canvas and the preview narrowed to a private 72
/// instead, so the same rail was a quarter wider anywhere but at run time.
/// One constant, read by every surface (the shell's
/// `MENU_PANE_COLLAPSED_WIDTH` is this constant re-exported), so they cannot
/// disagree again.
pub const COLLAPSED_WIDTH: f32 = 48.0;

/// How wide the rail is SHOWN when it is in the given state.
///
/// Open, it is as wide as the developer drew it. Collapsed, it is
/// [`COLLAPSED_WIDTH`] — an icon rail is a fixed width, not a proportion of
/// what it was. The designed rect is untouched either way: collapsing is a
/// state the rail is *in*, never an edit to the design.
pub fn shown_width(ctrl: &Control, collapsed: bool) -> f32 {
    if collapsed {
        COLLAPSED_WIDTH
    } else {
        (ctrl.rect.w as f32).max(1.0)
    }
}

const ROW_H: f32 = 42.0;
const ROW_GAP: f32 = 4.0;
const SECTION_H: f32 = 30.0;
/// Default header/footer pane heights; both are `HeaderHeight`/`FooterHeight`
/// properties on the control.
///
/// The header height is the DEVELOPER'S: it is read from `HeaderHeight` and
/// used as given on every surface. Nothing at run time recomputes it, scales it
/// to the window, or trims it to fit its contents — the rail's header is as
/// tall as it was drawn, and the logo box inside it is what adapts.
pub const HEADER_H: f32 = 120.0;
pub const FOOTER_H: f32 = 72.0;
const PAD_X: f32 = 12.0;

/// How far one level of nesting shifts a row right (operator, 2026-08-17).
///
/// It shifts the WHOLE row — the icon and the label together — so a group's
/// children read as a block set in from their group, and an item that has an
/// icon keeps that icon the same distance from its own label at every depth.
/// A brief attempt at holding one shared label column, with only the icon
/// stepping in, is what this replaced: it left an icon adrift between two
/// columns and no indentation to read the structure by.
///
/// Applied per level, so level two is set in twice as far as level one.
const NEST_INDENT: f32 = 16.0;
const ICON: f32 = 22.0;
const RADIUS: f32 = 10.0;

/// The header logo's box, in points — the **largest** a logo is drawn at when
/// the rail is open (operator, 2026-08-18).
///
/// A `HeaderImage` is drawn at its OWN size up to this, and scaled down to fit
/// inside it — keeping its aspect ratio — when it is bigger. A header with no
/// image outlines the box, so the developer designs their logo against a size
/// they can see instead of discovering it when the application runs.
///
/// It used to be 200×60 and the image was *stretched* to fill it exactly, so a
/// logo of any other shape arrived distorted and one drawn larger was squeezed
/// rather than fitted.
pub const HEADER_IMAGE_W: f32 = 270.0;
pub const HEADER_IMAGE_H: f32 = 80.0;

/// The COLLAPSED rail's header mark, drawn at this size (`HeaderIcon`).
///
/// A logo drawn for a 200pt header is illegible squeezed into a 72pt rail, so
/// the rail shows a purpose-made icon rather than a shrunken image. Centred
/// horizontally on the rail and vertically in the header pane — whose height
/// the rail keeps in both states, so the mark does not jump when it collapses.
pub const HEADER_ICON: f32 = 45.0;

/// What a laid-out row IS — the caller needs this to know what a click means.
#[derive(Clone, Debug, PartialEq)]
pub enum RowKind {
    /// The header pane: the logo, and the whole pane is the collapse toggle.
    Header,
    /// A section title (`HOME`) expanded, an ellipsis divider collapsed.
    Section(String),
    /// A menu item. `path` indexes into the definition (`[2]`, `[2, 0]`).
    Item { id: String, path: Vec<usize>, depth: usize },
    /// The footer pane. Its background is painted here; the Panel control that
    /// lives inside it is rendered by the form's own container machinery, so
    /// whatever the developer dropped in it draws normally.
    Footer,
}

/// One laid-out row and where it landed.
#[derive(Clone, Debug)]
pub struct SidebarRow {
    pub kind: RowKind,
    /// The row's full geometry — what the painter draws.
    pub rect: Rect,
    /// The part of it the operator can actually see and click: `rect` clipped
    /// to the pane the row belongs to. For a menu row scrolled half out of the
    /// menu pane these differ, and hit-testing must use THIS one — otherwise
    /// the half of the row hidden under the header still takes clicks.
    pub visible: Rect,
}

impl SidebarRow {
    fn whole(kind: RowKind, rect: Rect) -> Self {
        Self { kind, rect, visible: rect }
    }
}

/// The chrome around the menu itself, read from the SideMenu's properties.
#[derive(Clone, Debug)]
pub struct SidebarChrome {
    pub app_title: String,
    /// Header pane height in points.
    pub header_h: f32,
    /// Footer pane height in points. `0` means no footer pane.
    pub footer_h: f32,
}

impl Default for SidebarChrome {
    fn default() -> Self {
        Self {
            app_title: String::new(),
            header_h: HEADER_H,
            footer_h: FOOTER_H,
        }
    }
}

impl SidebarChrome {
    /// Read the chrome properties off a SideMenu control.
    pub fn from_control(ctrl: &Control) -> Self {
        let num = |k: &str, dflt: f32| {
            ctrl.get_prop(k)
                .map(|v| v.as_i64() as f32)
                .filter(|v| *v >= 0.0)
                .unwrap_or(dflt)
        };
        Self {
            app_title: ctrl
                .get_prop("AppTitle")
                .map(|v| v.as_str().to_owned())
                .unwrap_or_default(),
            header_h: num("HeaderHeight", HEADER_H),
            footer_h: num("FooterHeight", FOOTER_H),
        }
    }
}

/// Resolved colours — every one of them comes from the control.
#[derive(Clone, Copy, Debug)]
pub struct SidebarPalette {
    pub fg: Color32,
    pub dim: Color32,
    pub accent: Color32,
    pub on_accent: Color32,
    pub hover: Color32,
}

impl SidebarPalette {
    pub fn from_control(ctrl: &Control, alpha: u8) -> Self {
        let pick = |key: &str, fallback: Color32| -> Color32 {
            ctrl.get_prop(key)
                .map(|v| crate::paint::parse_color(v.as_str()))
                .filter(|c| c.a() > 0)
                .unwrap_or(fallback)
        };
        let fg = pick("ForegroundColor", Color32::from_rgb(225, 230, 250));
        let accent = pick("SelectedBgColor", Color32::from_rgb(0x53, 0x6d, 0xfe));
        let on_accent = pick("SelectedFgColor", Color32::WHITE);
        let hover = pick("HighlightBgColor", accent);
        let a = |c: Color32, mul: f32| {
            Color32::from_rgba_unmultiplied(
                c.r(),
                c.g(),
                c.b(),
                (alpha as f32 * mul) as u8,
            )
        };
        Self {
            fg: a(fg, 1.0),
            dim: a(fg, 0.55),
            accent: a(accent, 1.0),
            on_accent: a(on_accent, 1.0),
            hover: a(hover, 0.22),
        }
    }
}

/// Everything the painter needs that is not geometry.
pub struct SidebarState<'a> {
    pub items: &'a [MenuItem],
    pub collapsed: bool,
    pub chrome: SidebarChrome,
    pub palette: SidebarPalette,
    /// The item id drawn as active (the wide accent pill).
    pub selected: Option<&'a str>,
    /// Item ids whose children are expanded in place.
    pub expanded: &'a [String],
    /// The row the pointer is over, as an index into the laid-out rows.
    pub hovered: Option<usize>,
    /// Style for menu-item icons (the SideMenu's `IconEffect`).
    pub icon_style: crate::icons::IconStyle,
    /// Menu-item icon size in points while the rail is OPEN (the control's
    /// `IconSize`). Icons are vectors, so this scales cleanly at any value.
    pub icon_size: f32,
    /// Menu-item icon size in points while the rail is COLLAPSED (the
    /// control's `IconSizeCollapsed`).
    ///
    /// Its own size because the two states are two designs: open, the icon sits
    /// beside a label and must not overpower it; collapsed, the icon IS the row.
    /// BOTH are carried, and [`Self::collapsed`] picks between them at paint
    /// time — a caller that flips the state after building this state gets the
    /// matching size without rebuilding.
    pub icon_size_collapsed: f32,
    /// How far the MENU pane is scrolled, in points. The header and footer
    /// panes never move — only the rows between them. A rail with more items
    /// than height used to simply drop the overflow on the floor; now the
    /// operator can reach it.
    pub scroll: f32,
    /// The rail's own background, exactly as designed (`BackgroundColor`).
    /// Routinely TRANSLUCENT, which is the point: it composites over
    /// [`Self::backdrop`], the way the designer canvas composites a control
    /// over the form it sits on.
    pub bg: Color32,
    /// What the rail is painted ON — the form's own backdrop. Every surface
    /// passes the same one, so a translucent rail colour cannot resolve to a
    /// different shade depending on where you look at it.
    pub backdrop: Color32,
    /// The rail's background gradient (start, end, direction) when the control
    /// enables one. It REPLACES [`Self::bg`] rather than layering over it —
    /// the same rule the form background follows.
    pub gradient: Option<(Color32, Color32, String)>,
    /// The header logo (`HeaderImage`), resolved to a texture. `None` draws no
    /// logo at all — a placeholder mark is decoration standing in for content.
    pub header_image: Option<egui::TextureId>,
    /// The COLLAPSED rail's header mark (`HeaderIcon`), an image resolved to a
    /// texture — its own file, picked from disk like the logo.
    pub header_icon: Option<egui::TextureId>,
    /// The rail's drop shadow (`ShadowEnabled` and its companions), resolved
    /// here for the same reason the logo is: `draw_control` hands the whole
    /// SideMenu over to [`paint`], and the shell and the preview never call
    /// `draw_control` at all — so a shadow left to the generic control frame
    /// was drawn on no surface whatsoever.
    pub shadow: Option<crate::paint::DropShadowSpec>,
    pub font: FontId,
}

/// Build the state for a SideMenu control — palette, chrome, icon style and
/// font all read from the control itself. The ONE place those are derived, so
/// the canvas, the preview, Run Form and the shell cannot disagree.
///
/// `expanded` lists the parent ids whose children are shown. Pass an empty
/// slice for "all closed"; the designer canvas passes every parent so the
/// developer sees the whole tree without running the application.
pub fn state_for_control<'a>(
    ctx: &egui::Context,
    ctrl: &'a Control,
    items: &'a [MenuItem],
    alpha: u8,
    expanded: &'a [String],
) -> SidebarState<'a> {
    let palette = SidebarPalette::from_control(ctrl, alpha);
    // The rail's own default shadow DIRECTION is East, not the generic South.
    //
    // A rail is full-height, so a shadow thrown South lands below the form —
    // off-surface, clipped, invisible. Switching `ShadowEnabled` on therefore
    // appeared to do nothing at all, which is exactly how it was reported. East
    // casts it onto the content area, along the edge the rail actually has. Any
    // direction the developer sets still wins.
    let shadow_source: Control = match ctrl
        .get_prop("ShadowDirection")
        .map(|v| v.as_str().trim().to_owned())
        .filter(|s| !s.is_empty())
    {
        Some(_) => ctrl.clone(),
        None => {
            let mut c = ctrl.clone();
            c.set_prop("ShadowDirection", "East");
            c
        }
    };
    let font_name = ctrl
        .get_prop("FontName")
        .map(|v| v.as_str())
        .unwrap_or_default();
    let font = crate::fonts::font_id(ctx, &font_name, crate::paint::ctrl_font_size(ctrl));
    SidebarState {
        items,
        collapsed: ctrl.side_menu_collapsed(),
        chrome: SidebarChrome::from_control(ctrl),
        palette,
        selected: ctrl
            .get_prop("SelectedItemId")
            .map(|v| v.as_str())
            .filter(|s| !s.is_empty()),
        expanded,
        hovered: None,
        icon_style: crate::icons::icon_style_for_effect(
            ctrl.get_prop("IconEffect")
                .map(|v| v.as_str())
                .unwrap_or("None"),
            palette.fg,
        ),
        icon_size: icon_size_prop(ctrl, "IconSize").unwrap_or(ICON),
        // A form designed before the collapsed size existed falls back to the
        // OPEN one, which is exactly how it looked then — a new property may
        // not restyle a rail nobody has touched.
        icon_size_collapsed: icon_size_prop(ctrl, "IconSizeCollapsed")
            .or_else(|| icon_size_prop(ctrl, "IconSize"))
            .unwrap_or(ICON),
        scroll: 0.0,
        // `parse_color` yields a PREMULTIPLIED colour, so fading it means
        // scaling all four channels — rebuilding it as straight alpha would
        // premultiply a second time and darken the rail.
        bg: ctrl
            .get_prop("BackgroundColor")
            .map(|v| crate::paint::parse_color(v.as_str()))
            .map(|c| {
                let k = alpha as f32 / 255.0;
                Color32::from_rgba_premultiplied(
                    (c.r() as f32 * k) as u8,
                    (c.g() as f32 * k) as u8,
                    (c.b() as f32 * k) as u8,
                    (c.a() as f32 * k) as u8,
                )
            })
            .unwrap_or(Color32::TRANSPARENT),
        backdrop: Color32::TRANSPARENT,
        gradient: ctrl
            .get_prop("BackgroundGradientEnabled")
            .map(|v| v.as_bool())
            .unwrap_or(false)
            .then(|| {
                let k = alpha as f32 / 255.0;
                let pick = |key: &str, dflt: Color32| {
                    let c = ctrl
                        .get_prop(key)
                        .map(|v| crate::paint::parse_color(v.as_str()))
                        .filter(|c| c.a() > 0)
                        .unwrap_or(dflt);
                    Color32::from_rgba_premultiplied(
                        (c.r() as f32 * k) as u8,
                        (c.g() as f32 * k) as u8,
                        (c.b() as f32 * k) as u8,
                        (c.a() as f32 * k) as u8,
                    )
                };
                (
                    pick("BackgroundGradientStartColor", Color32::from_rgb(0xF0, 0xF0, 0xF0)),
                    pick("BackgroundGradientEndColor", Color32::from_rgb(0xC8, 0xD0, 0xDC)),
                    ctrl.get_prop("BackgroundGradientDirection")
                        .map(|v| v.as_str().to_owned())
                        .unwrap_or_else(|| "South".to_owned()),
                )
            }),
        // The logo is resolved HERE, once, for every surface at once. Leaving
        // each surface to do it is why setting `HeaderImage` stored a path and
        // drew nothing, on all four of them.
        header_image: ctrl
            .get_prop("HeaderImage")
            .map(|v| v.as_str().to_owned())
            .filter(|p| !p.trim().is_empty())
            .and_then(|p| crate::paint::cached_image_texture(ctx, &p))
            .map(|t| t.id()),
        header_icon: ctrl
            .get_prop("HeaderIcon")
            .map(|v| v.as_str().to_owned())
            .filter(|p| !p.trim().is_empty())
            .and_then(|p| crate::paint::cached_image_texture(ctx, &p))
            .map(|t| t.id()),
        // The control's alpha is folded in ONCE, here, so the painter places
        // the shadow without having to know anything about fading.
        //
        // 050 R4/R6 — the neumorphic question goes through the gate like every
        // other painting read of the glass style. `drop_shadow_spec` returns
        // `None` when neumorphic, so an ungated read here would suppress the
        // rail's drop shadow under a self-contained theme that has no relief to
        // replace it with.
        shadow: crate::paint::drop_shadow_spec(
            &shadow_source,
            crate::paint::glass_config_applies(ctx)
                && crate::paint::active_glass_style(ctx).is_neumorphic(),
        )
        .map(|s| s.faded(alpha as f32 / 255.0)),
        font,
    }
}

/// One of the two icon-size properties, in points — `None` when it is absent
/// or too small to draw, which is what lets the collapsed size fall back to
/// the open one instead of to a constant.
fn icon_size_prop(ctrl: &Control, key: &str) -> Option<f32> {
    ctrl.get_prop(key)
        .map(|v| v.as_i64() as f32)
        .filter(|s| *s >= 4.0)
}

/// Every parent id in `items`, for surfaces that show the tree open.
pub fn all_parent_ids(items: &[MenuItem]) -> Vec<String> {
    fn walk(items: &[MenuItem], out: &mut Vec<String>) {
        for i in items {
            if i.has_children() {
                out.push(i.id.clone());
                walk(&i.items, out);
            }
        }
    }
    let mut out = Vec::new();
    walk(items, &mut out);
    out
}

/// Lay the rail out without painting: the row rectangles, in draw order.
/// Hit-testing and painting both walk this, so a click can never land on a
/// different row than the one drawn under the pointer.
pub fn layout(rect: Rect, state: &SidebarState<'_>) -> Vec<SidebarRow> {
    let band = menu_band(rect, state);
    let mut rows = Vec::new();

    // ── Header pane ─────────────────────────────────────────────────────────
    // Full width, the logo, and the whole pane is the collapse toggle. It does
    // not scroll: the toggle has to stay reachable however long the menu is.
    rows.push(SidebarRow::whole(
        RowKind::Header,
        Rect::from_min_size(rect.min, Vec2::new(rect.width(), state.chrome.header_h)),
    ));

    // ── Menu pane ───────────────────────────────────────────────────────────
    // Every row is laid out, whether or not it fits; the ones outside the band
    // are dropped AFTER the walk, so scrolling reaches them. Dropping them
    // during the walk (which is what this did) is exactly why the rail clipped
    // its own menu with no way to see the rest.
    let mut y = band.min.y - clamp_scroll(rect, state);
    if state.collapsed {
        rail_rows(state.items, &mut y, rect, &mut rows);
    } else {
        let mut prefix = Vec::new();
        walk_rows(state.items, &mut prefix, 0, &mut y, rect, state, &mut rows);
    }

    // ── Footer pane ─────────────────────────────────────────────────────────
    // Anchored to the bottom, full width, and hidden on the collapsed rail —
    // whatever the developer dropped into its Panel will not fit at rail
    // width, so the icons get the height instead. It does not scroll either.
    if footer_h(state) > 0.0 {
        rows.push(SidebarRow::whole(
            RowKind::Footer,
            Rect::from_min_size(
                Pos2::new(rect.min.x, rect.max.y - footer_h(state)),
                Vec2::new(rect.width(), footer_h(state)),
            ),
        ));
    }

    // Menu rows are confined to their band: a row wholly outside is gone, and
    // one straddling an edge keeps its geometry for the painter but only its
    // visible part for the pointer.
    rows.retain(|r| match r.kind {
        RowKind::Header | RowKind::Footer => true,
        _ => r.rect.intersects(band),
    });
    for r in &mut rows {
        if !matches!(r.kind, RowKind::Header | RowKind::Footer) {
            r.visible = r.rect.intersect(band);
        }
    }
    rows
}

/// The footer pane's height — the developer's, in BOTH rail states.
///
/// The collapsed rail used to drop the footer entirely and hand its height to
/// the icons. That made the three panes move under the operator every time
/// they collapsed the rail, and it contradicted the rule that these heights are
/// the developer's and nothing at run time may change them.
fn footer_h(state: &SidebarState<'_>) -> f32 {
    state.chrome.footer_h
}

/// The band the scrolling menu rows live in — the rail minus its header and
/// footer panes.
pub fn menu_band(rect: Rect, state: &SidebarState<'_>) -> Rect {
    Rect::from_min_max(
        Pos2::new(rect.min.x, rect.min.y + state.chrome.header_h),
        Pos2::new(rect.max.x, rect.max.y - footer_h(state)),
    )
}

/// How tall the menu rows are in total, scroll or no scroll.
pub fn menu_content_height(rect: Rect, state: &SidebarState<'_>) -> f32 {
    let mut y = 0.0_f32;
    let mut prefix = Vec::new();
    let mut sink = Vec::new();
    walk_rows(state.items, &mut prefix, 0, &mut y, rect, state, &mut sink);
    y.max(0.0)
}

/// How far the menu pane CAN scroll: zero when everything already fits.
pub fn max_scroll(rect: Rect, state: &SidebarState<'_>) -> f32 {
    (menu_content_height(rect, state) - menu_band(rect, state).height()).max(0.0)
}

/// The state's scroll, held inside what there is to scroll.
pub fn clamp_scroll(rect: Rect, state: &SidebarState<'_>) -> f32 {
    state.scroll.clamp(0.0, max_scroll(rect, state))
}

/// The controls to PAINT for a shell form whose rail is shown collapsed: the
/// SideMenu and the footer Panel it owns, narrowed to the rail width, with the
/// rail's live `Collapsed` state written on. Every other control is returned
/// untouched.
///
/// A rail shown collapsed must be DRAWN at the collapsed width. Keeping the
/// designed width and merely laying collapsed content out inside it gives a
/// full-width bar of icon-only rows — the rail looks open and behaves closed,
/// and the breadcrumb beside it, which positions from the rail width, lands in
/// the middle of it.
///
/// Shared so every surface narrows identically. It existed only inside the
/// designer canvas, which is exactly why the preview drew the wide-bar version
/// while the canvas and the running shell agreed.
///
/// The DESIGN is never touched: this list is for painting, and selection,
/// dragging and the saved `.cfrm` all still see the designed rect.
pub fn rail_view(controls: &[Control], side: &Control, collapsed: bool) -> Vec<Control> {
    let footer_id = crate::model::side_menu_footer_id(&side.id);
    let width = shown_width(side, collapsed) as i32;
    controls
        .iter()
        .map(|c| {
            if c.id != side.id && c.id != footer_id {
                return c.clone();
            }
            let mut c = c.clone();
            // The footer Panel is pinned to the rail's column, so it narrows
            // with it — otherwise it hangs out over the content area the
            // moment the rail collapses.
            c.rect.w = width;
            if c.id == side.id {
                c.set_prop("Collapsed", collapsed);
            }
            c
        })
        .collect()
}

/// Lay out the menu rows from `y` downwards, recursing into expanded parents.
/// Rows are produced unconditionally — confining them to the pane is the
/// Whether an item earns a place on the COLLAPSED rail (operator, 2026-08-17).
///
/// The rail is one icon wide, so an item has to be reachable BY its icon. Three
/// things follow, and they are the whole rule:
///
/// * **It must have an icon.** An item with an action but no icon has nothing to
///   draw and nothing to aim at — a blank row that does something when clicked.
/// * **It must do something.** An item with no action is a label, and a rail has
///   no room for labels.
/// * **It must not be a group.** A group's whole meaning is the list it opens,
///   and there is nowhere on a rail to open one to. Its qualifying CHILDREN come
///   up in its place instead ([`rail_rows`]), so nothing reachable is lost —
///   which is the point: the rail is the shortcuts, not the structure.
///
/// Deliberately not special-cased by label: "Home" appears because it has an
/// icon and an action like anything else, not because of what it is called.
/// A rule that reads names would break the moment the menu is in Portuguese.
pub fn shows_on_rail(item: &MenuItem) -> bool {
    let filled = |s: &Option<String>| s.as_deref().map(|v| !v.trim().is_empty()).unwrap_or(false);
    item.item_type != MenuItemType::Separator
        && !item.has_children()
        && filled(&item.icon)
        && filled(&item.action)
}

/// Lay out the COLLAPSED rail: every item that [`shows_on_rail`] accepts, at
/// whatever depth it sits, flattened to one column of icons.
///
/// Each row keeps its TRUE path into the item tree, so a child surfaced from
/// inside a group still resolves through [`item_at`] and still belongs to the
/// slot its top-level ancestor belongs to — which is how the shell tells a root
/// menu click from an open form's.
///
/// A divider is emitted only BETWEEN two icons. That drops the section titles
/// that no longer separate anything now the groups are gone, and keeps the one
/// that still means something: the rule between the application's own menu and
/// the operations of the form that is open.
fn rail_rows(items: &[MenuItem], y: &mut f32, rect: Rect, rows: &mut Vec<SidebarRow>) {
    enum Entry {
        Icon { id: String, path: Vec<usize>, home: bool },
        Divider(String),
    }
    fn walk(items: &[MenuItem], prefix: &mut Vec<usize>, out: &mut Vec<Entry>) {
        for (i, item) in items.iter().enumerate() {
            prefix.push(i);
            if item.item_type == MenuItemType::Separator {
                // The label rides along: the rail draws an ellipsis, but the row
                // still knows which section it came from.
                out.push(Entry::Divider(item.section_title().unwrap_or_default().to_owned()));
            } else if shows_on_rail(item) {
                out.push(Entry::Icon {
                    id: item.id.clone(),
                    path: prefix.clone(),
                    home: is_home(item),
                });
            }
            // A group contributes nothing itself, but what is INSIDE it can.
            if item.has_children() {
                walk(&item.items, prefix, out);
            }
            prefix.pop();
        }
    }
    let mut entries = Vec::new();
    walk(items, &mut Vec::new(), &mut entries);

    // Keep a divider only where an icon precedes it and an icon follows.
    let mut keep = vec![false; entries.len()];
    let mut seen_icon = false;
    for (i, e) in entries.iter().enumerate() {
        match e {
            Entry::Icon { .. } => {
                keep[i] = true;
                seen_icon = true;
            }
            Entry::Divider(_) => {
                keep[i] = seen_icon
                    && entries[i + 1..]
                        .iter()
                        .any(|later| matches!(later, Entry::Icon { .. }));
            }
        }
    }
    // …and only ONE where several fell together. The survivors are collected
    // first, because laying a row out needs to know what comes AFTER it.
    let mut final_entries: Vec<&Entry> = Vec::new();
    let mut last_was_divider = false;
    for (i, e) in entries.iter().enumerate() {
        if !keep[i] {
            continue;
        }
        match e {
            Entry::Divider(_) if last_was_divider => continue,
            Entry::Divider(_) => last_was_divider = true,
            Entry::Icon { .. } => last_was_divider = false,
        }
        final_entries.push(e);
    }

    for (i, e) in final_entries.iter().enumerate() {
        match e {
            Entry::Divider(label) => {
                rows.push(SidebarRow::whole(
                    RowKind::Section(label.clone()),
                    Rect::from_min_size(Pos2::new(rect.min.x, *y), Vec2::new(rect.width(), SECTION_H)),
                ));
                *y += SECTION_H;
            }
            Entry::Icon { id, path, home } => {
                rows.push(SidebarRow::whole(
                    RowKind::Item {
                        id: id.clone(),
                        path: path.clone(),
                        // A rail has one column; indenting an icon by how deep
                        // it used to sit would only push it off centre.
                        depth: 0,
                    },
                    Rect::from_min_size(Pos2::new(rect.min.x, *y), Vec2::new(rect.width(), ROW_H)),
                ));
                *y += ROW_H + ROW_GAP;
                // HOME stands apart from the icons under it (operator,
                // 2026-08-17): a whole row's worth of extra space, so the
                // distance from Home to the next icon is exactly twice the
                // distance between any other two.
                //
                // Only where an icon actually follows — a divider already
                // separates, and trailing space at the foot of the rail
                // separates nothing.
                if *home && matches!(final_entries.get(i + 1), Some(Entry::Icon { .. })) {
                    *y += ROW_H + ROW_GAP;
                }
            }
        }
    }
}

/// Whether an item is the shell's HOME (spec 051), by its ACTION.
///
/// Never by its label: the action is the same string in every language, and a
/// rule that read "Home" would stop working the moment the menu was written in
/// Portuguese — the same reason [`shows_on_rail`] special-cases nothing.
pub fn is_home(item: &MenuItem) -> bool {
    item.action.as_deref() == Some("home")
}

/// caller's job, and doing it here is what lost the overflow.
fn walk_rows(
    items: &[MenuItem],
    prefix: &mut Vec<usize>,
    depth: usize,
    y: &mut f32,
    rect: Rect,
    state: &SidebarState<'_>,
    rows: &mut Vec<SidebarRow>,
) {
    for (i, item) in items.iter().enumerate() {
        prefix.push(i);
        if item.item_type == MenuItemType::Separator {
            let h = if item.section_title().is_some() {
                SECTION_H
            } else {
                ROW_GAP * 2.0
            };
            rows.push(SidebarRow::whole(
                RowKind::Section(item.section_title().unwrap_or_default().to_owned()),
                Rect::from_min_size(Pos2::new(rect.min.x, *y), Vec2::new(rect.width(), h)),
            ));
            *y += h;
            prefix.pop();
            continue;
        }
        rows.push(SidebarRow::whole(
            RowKind::Item {
                id: item.id.clone(),
                path: prefix.clone(),
                depth,
            },
            Rect::from_min_size(Pos2::new(rect.min.x, *y), Vec2::new(rect.width(), ROW_H)),
        ));
        *y += ROW_H + ROW_GAP;
        // Children show only when expanded, and never on the rail.
        if !state.collapsed && item.has_children() && state.expanded.iter().any(|e| e == &item.id) {
            walk_rows(&item.items, prefix, depth + 1, y, rect, state, rows);
        }
        prefix.pop();
    }
}

/// The footer pane's rect for a sidebar occupying `rect` — where the footer
/// Panel control is laid out. `None` when the rail is collapsed or the footer
/// height is zero.
pub fn footer_rect(rect: Rect, chrome: &SidebarChrome, _collapsed: bool) -> Option<Rect> {
    if chrome.footer_h <= 0.0 {
        return None;
    }
    Some(Rect::from_min_size(
        Pos2::new(rect.min.x, rect.max.y - chrome.footer_h),
        Vec2::new(rect.width(), chrome.footer_h),
    ))
}

/// Which laid-out row contains `pos`, if any. Tests the row's VISIBLE part, so
/// a menu row scrolled half under the header does not take a click there.
pub fn row_at(rows: &[SidebarRow], pos: Pos2) -> Option<usize> {
    rows.iter()
        .position(|r| r.visible.is_positive() && r.visible.contains(pos))
}

/// Paint the rail into `rect`. `rows` must come from [`layout`] for the same
/// rect and state.
///
/// The rail's BACKGROUND is painted here, from the control's own
/// `BackgroundColor` over the surface's backdrop. It used to be every
/// surface's own business: the canvas got it from the generic control frame
/// (with a glass border the rail never wanted), the preview drew none at all,
/// and the shell composed its own — three different rails for one design.
pub fn paint(
    painter: &egui::Painter,
    rect: Rect,
    rows: &[SidebarRow],
    state: &SidebarState<'_>,
) {
    // The rail's own drop shadow, UNDER its face — the ordinary place for one.
    // It is drawn here rather than by the generic control frame because this is
    // the only code all four surfaces run.
    if let Some(sh) = state.shadow.filter(|s| !s.is_overlay()) {
        sh.paint(painter, rect, 1.0);
    }
    match &state.gradient {
        // A gradient REPLACES the plain colour. It still lands on the form's
        // backdrop first, so a translucent gradient shows the application
        // through it rather than the desktop.
        Some((start, end, dir)) => {
            if state.backdrop.a() > 0 {
                painter.rect_filled(rect, 0.0, state.backdrop);
            }
            painter.add(egui::Shape::mesh(crate::paint::background_gradient_mesh(
                rect,
                *start,
                *end,
                dir,
                egui::CornerRadius::ZERO,
            )));
        }
        None => {
            let fill = crate::paint::composite_premultiplied_over(state.bg, state.backdrop);
            if fill.a() > 0 {
                painter.rect_filled(rect, 0.0, fill);
            }
        }
    }
    for (ix, row) in rows.iter().enumerate() {
        let hovered = state.hovered == Some(ix);
        // A scrolled menu row keeps its full geometry — the pill, the icon and
        // the label are drawn where they belong — and is cut to the pane by a
        // clip, so it slides under the header and footer instead of overrunning
        // them.
        let p = painter.with_clip_rect(row.visible);
        match &row.kind {
            RowKind::Header => paint_header(painter, row.rect, state),
            RowKind::Section(title) => paint_section(&p, row.rect, title, state),
            RowKind::Item { id, path, depth } => {
                if let Some(item) = item_at(state.items, path) {
                    let active = state.selected == Some(id.as_str());
                    paint_item(&p, row.rect, item, *depth, active, hovered, state);
                }
            }
            RowKind::Footer => paint_footer(painter, row.rect, state),
        }
    }
    // A NEGATIVE `ShadowBlurStrength` is the sunken variant: it goes over the
    // face, after the rows, exactly as it does for every other control.
    if let Some(sh) = state.shadow.filter(|s| s.is_overlay()) {
        sh.paint(painter, rect, 1.0);
    }
}

/// Resolve a layout path back to its item.
pub fn item_at<'a>(items: &'a [MenuItem], path: &[usize]) -> Option<&'a MenuItem> {
    let mut cur = items;
    let mut out = None;
    for &ix in path {
        let it = cur.get(ix)?;
        out = Some(it);
        cur = &it.items;
    }
    out
}

/// The logo's box inside a header pane occupying `header`.
///
/// [`HEADER_IMAGE_W`] × [`HEADER_IMAGE_H`], shrunk to fit — and shrunk keeping
/// its 10:3 shape, so a logo drawn for the box is never distorted by the rail
/// being narrow (the collapsed rail is a third of the box's width) or the
/// header being shorter than the box is tall.
pub fn header_image_rect(header: Rect, collapsed: bool) -> Rect {
    let avail_w = (header.width() - PAD_X * 2.0).max(1.0);
    let avail_h = (header.height() - 8.0).max(1.0);
    let k = (avail_w / HEADER_IMAGE_W)
        .min(avail_h / HEADER_IMAGE_H)
        .min(1.0);
    let size = Vec2::new(HEADER_IMAGE_W * k, HEADER_IMAGE_H * k);
    // Centred in the header pane, both axes and in both rail states. It used to
    // be pinned to the left padding when expanded, which left the logo sitting
    // off-centre against the pane it is the whole content of.
    let _ = collapsed;
    Rect::from_min_size(
        Pos2::new(
            header.center().x - size.x * 0.5,
            header.center().y - size.y * 0.5,
        ),
        size,
    )
}

fn paint_header(painter: &egui::Painter, rect: Rect, state: &SidebarState<'_>) {
    let pal = state.palette;
    // The header carries the developer's OWN logo, stretched to fill the box
    // exactly — the box is the contract, so what they see designing is what
    // ships. With no image the box is OUTLINED rather than filled with an
    // invented mark: it shows where the logo goes and how big it will be
    // without standing in for content that does not exist.
    // Collapsed, the rail shows its ICON, not a shrunken logo: an image drawn
    // for a 200pt header cannot be read at rail width. 45x45, centred on the
    // rail and in the middle of the header pane — whose height the rail keeps
    // in both states, so the mark does not move when it collapses.
    if state.collapsed {
        if let Some(tex) = state.header_icon {
            painter.image(
                tex,
                Rect::from_center_size(rect.center(), Vec2::splat(HEADER_ICON)),
                Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
        }
        return;
    }

    let logo = header_image_rect(rect, state.collapsed);
    match state.header_image {
        Some(tex) => {
            // The box is a LIMIT, not a shape to fill: the logo is drawn at its
            // own size while it fits, and scaled down keeping its aspect ratio
            // when it does not. Stretching it to the box, as this used to,
            // distorted every logo that was not exactly 10:3.
            let native = painter
                .ctx()
                .tex_manager()
                .read()
                .meta(tex)
                .map(|m| Vec2::new(m.size[0] as f32, m.size[1] as f32))
                .unwrap_or_else(|| logo.size());
            painter.image(
                tex,
                crate::paint::media_dest_rect(logo, native, "Center"),
                Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
        }
        None => {
            let c = [
                logo.left_top(),
                logo.right_top(),
                logo.right_bottom(),
                logo.left_bottom(),
                logo.left_top(),
            ];
            painter.extend(egui::Shape::dashed_line(
                &c,
                Stroke::new(1.0, pal.dim),
                4.0,
                4.0,
            ));
        }
    }

    if !state.collapsed && !state.chrome.app_title.trim().is_empty() {
        // The title follows the logo box, and only when there is room left for
        // it — a 200pt logo fills most of a rail on its own.
        let x = logo.max.x + 10.0;
        if rect.max.x - PAD_X - x > 8.0 {
            let mut f = state.font.clone();
            f.size *= 1.45;
            painter
                .with_clip_rect(Rect::from_min_max(
                    Pos2::new(x, rect.min.y),
                    Pos2::new(rect.max.x - PAD_X, rect.max.y),
                ))
                .text(
                    Pos2::new(x, rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    state.chrome.app_title.trim(),
                    f,
                    pal.accent,
                );
        }
    }
    // NO hamburger glyph: the header pane carries the logo, and clicking
    // anywhere in it collapses or opens the rail. A ☰ would be a second
    // affordance for something the whole pane already does.
}

fn paint_section(painter: &egui::Painter, rect: Rect, title: &str, state: &SidebarState<'_>) {
    let pal = state.palette;
    if title.is_empty() {
        // Unlabelled separator: a hairline rule, both states.
        let y = rect.center().y;
        painter.line_segment(
            [
                Pos2::new(rect.min.x + PAD_X, y),
                Pos2::new(rect.max.x - PAD_X, y),
            ],
            Stroke::new(1.0, pal.dim),
        );
        return;
    }
    if state.collapsed {
        // The rail divides groups with an ellipsis instead of a title.
        let cy = rect.center().y;
        for k in -1..=1 {
            painter.circle_filled(
                Pos2::new(rect.center().x + k as f32 * 5.0, cy),
                1.3,
                pal.dim,
            );
        }
        return;
    }
    let mut f = state.font.clone();
    f.size *= 0.82;
    painter.text(
        Pos2::new(rect.min.x + PAD_X + 4.0, rect.center().y + 4.0),
        egui::Align2::LEFT_CENTER,
        title.to_uppercase(),
        f,
        pal.dim,
    );
}

/// How far a row at `depth` is set in from the rail's left padding.
///
/// ONE offset for the whole row: [`item_icon_rect`] and [`item_label_x`] both
/// start from it, which is what keeps a child's icon and its label moving
/// together instead of drifting apart (operator, 2026-08-17).
fn nest_offset(depth: usize) -> f32 {
    depth as f32 * NEST_INDENT
}

/// The box a row's icon is drawn in. Centred on the rail when collapsed —
/// there is one column there, and indenting it would only push it off centre.
fn item_icon_rect(rect: Rect, depth: usize, icon: f32, collapsed: bool) -> Rect {
    let c = if collapsed {
        rect.center()
    } else {
        Pos2::new(
            rect.min.x + PAD_X + nest_offset(depth) + icon * 0.5,
            rect.center().y,
        )
    };
    Rect::from_center_size(c, Vec2::splat(icon))
}

/// Where a row's label starts — the icon box it follows, plus a gap. An item
/// with no icon still starts here, so a menu of mixed items keeps one text
/// column per level rather than one per item.
fn item_label_x(rect: Rect, depth: usize, icon: f32) -> f32 {
    rect.min.x + PAD_X + nest_offset(depth) + icon + 10.0
}

fn paint_item(
    painter: &egui::Painter,
    rect: Rect,
    item: &MenuItem,
    depth: usize,
    active: bool,
    hovered: bool,
    state: &SidebarState<'_>,
) {
    let pal = state.palette;
    let enabled = item.enabled;
    // The active row is a wide rounded rectangle in the accent colour; on the
    // rail it shrinks to a rounded square around the icon.
    let pill = if state.collapsed {
        Rect::from_center_size(rect.center(), Vec2::splat(ROW_H - 6.0))
    } else {
        Rect::from_min_max(
            Pos2::new(rect.min.x + PAD_X * 0.5, rect.min.y),
            Pos2::new(rect.max.x - PAD_X * 0.5, rect.max.y),
        )
    };
    if active {
        painter.rect_filled(pill, RADIUS, pal.accent);
    } else if hovered && enabled {
        painter.rect_filled(pill, RADIUS, pal.hover);
    }

    let content = if active { pal.on_accent } else if enabled { pal.fg } else { pal.dim };
    // Each rail state draws at its own size — the icon beside a label and the
    // icon that IS the row are two different pictures.
    let icon = if state.collapsed { state.icon_size_collapsed } else { state.icon_size };
    // A group's children are INDENTED, icon and label alike (operator,
    // 2026-08-17): the row moves as one, so the icon keeps its place beside its
    // own label wherever the item sits in the tree.
    let icon_box = item_icon_rect(rect, depth, icon, state.collapsed);
    let icon_c = icon_box.center();

    match &item.icon {
        Some(name) => {
            let mut style = state.icon_style;
            style.color = content;
            crate::icons::draw_menu_icon_styled(painter, icon_box, name, &style);
        }
        None if state.collapsed => {
            // Icon-only rail: the initial keeps an iconless item reachable.
            let initial = item.label.chars().next().map(String::from).unwrap_or_default();
            painter.text(
                icon_c,
                egui::Align2::CENTER_CENTER,
                initial,
                state.font.clone(),
                content,
            );
        }
        None => {}
    }

    if state.collapsed {
        // A badge on the rail shrinks to a dot in the corner.
        if item.badge_text().is_some() {
            painter.circle_filled(
                Pos2::new(pill.max.x - 6.0, pill.min.y + 6.0),
                3.5,
                if active { pal.on_accent } else { pal.accent },
            );
        }
        return;
    }

    // Label — indented by the same offset the icon took, so the two stay one
    // row and each level has its own text column.
    let label_x = item_label_x(rect, depth, icon);
    let mut right = rect.max.x - PAD_X;

    // Chevron for a row that owns children.
    if item.has_children() {
        let cx = right - 6.0;
        let cy = rect.center().y;
        let open = state.expanded.iter().any(|e| e == &item.id);
        let (dy0, dy1) = if open { (2.5, -2.5) } else { (-2.5, 2.5) };
        painter.add(egui::Shape::line(
            vec![
                Pos2::new(cx - 5.0, cy + dy0),
                Pos2::new(cx, cy + dy1),
                Pos2::new(cx + 5.0, cy + dy0),
            ],
            Stroke::new(1.6, content),
        ));
        right -= 20.0;
    }

    // Badge, right-aligned before the chevron.
    if let Some(text) = item.badge_text() {
        let mut bf = state.font.clone();
        bf.size *= 0.78;
        let galley = painter.layout_no_wrap(text.to_owned(), bf.clone(), content);
        let (bw, bh) = (galley.size().x, galley.size().y);
        match item.badge_style {
            BadgeStyle::Count => {
                let r = (bh * 0.5 + 5.0).max(9.0);
                let c = Pos2::new(right - r, rect.center().y);
                painter.circle_filled(c, r, if active { pal.on_accent } else { pal.accent });
                painter.text(
                    c,
                    egui::Align2::CENTER_CENTER,
                    text,
                    bf,
                    if active { pal.accent } else { pal.on_accent },
                );
                right -= r * 2.0 + 6.0;
            }
            BadgeStyle::Outline => {
                let bg = Rect::from_min_size(
                    Pos2::new(right - bw - 16.0, rect.center().y - bh * 0.5 - 3.0),
                    Vec2::new(bw + 16.0, bh + 6.0),
                );
                painter.rect_stroke(
                    bg,
                    bg.height() * 0.5,
                    Stroke::new(1.2, if active { pal.on_accent } else { pal.accent }),
                    egui::StrokeKind::Middle,
                );
                painter.text(
                    bg.center(),
                    egui::Align2::CENTER_CENTER,
                    text,
                    bf,
                    if active { pal.on_accent } else { pal.accent },
                );
                right -= bg.width() + 6.0;
            }
            BadgeStyle::Pill => {
                let bg = Rect::from_min_size(
                    Pos2::new(right - bw - 16.0, rect.center().y - bh * 0.5 - 3.0),
                    Vec2::new(bw + 16.0, bh + 6.0),
                );
                painter.rect_filled(
                    bg,
                    bg.height() * 0.5,
                    if active { pal.on_accent } else { pal.accent },
                );
                painter.text(
                    bg.center(),
                    egui::Align2::CENTER_CENTER,
                    text,
                    bf,
                    if active { pal.accent } else { pal.on_accent },
                );
                right -= bg.width() + 6.0;
            }
        }
    }

    // The label is clipped to whatever the badge and chevron left behind, so a
    // long label can never run over them (or out of the rail, as it did before
    // this module existed).
    let clip = Rect::from_min_max(
        Pos2::new(label_x, rect.min.y),
        Pos2::new(right.max(label_x), rect.max.y),
    );
    let p = painter.with_clip_rect(clip);
    p.text(
        Pos2::new(label_x, rect.center().y),
        egui::Align2::LEFT_CENTER,
        &item.label,
        state.font.clone(),
        content,
    );
}

/// The footer pane is a BAND, not a widget: it spans the full sidebar width
/// and the Panel control that lives in it — the one the developer drops
/// controls into — is drawn by the form's own container rendering. It paints
/// NOTHING of its own: the Panel's Background and Transparency (the only two
/// properties the developer may change on it) are what colour the footer, and
/// a separating rule drawn here was a border nobody asked for.
fn paint_footer(_painter: &egui::Painter, _rect: Rect, _state: &SidebarState<'_>) {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::menu::MenuItem;

    fn ctx() -> egui::Context {
        egui::Context::default()
    }

    /// The AdminMart-style menu these tests are laid out from.
    ///
    /// The leaf items carry an `action` as well as an icon, because a real one
    /// does: navigation is dispatched entirely from the action string
    /// (`home`, `open-form:…`), so an item without one does nothing when it is
    /// clicked. They were icon-only here until the collapsed rail started
    /// requiring both, which is a fixture that had drifted from what it stood
    /// for rather than a rule that was too strict.
    fn sample() -> Vec<MenuItem> {
        let mut home = MenuItem::new_separator("s1");
        home.label = "Home".into();
        let mut modern = MenuItem::new_action("modern", "Modern");
        modern.icon = Some("home".into());
        modern.action = Some("home".into());
        modern.badge = Some("New".into());
        let mut analytical = MenuItem::new_action("analytical", "Analytical");
        analytical.icon = Some("chart-bar".into());
        analytical.action = Some("open-form:ANALYTICS".into());
        let mut apps = MenuItem::new_separator("s2");
        apps.label = "Apps".into();
        let mut chat = MenuItem::new_action("chat", "Chat");
        chat.icon = Some("chat".into());
        chat.action = Some("open-form:CHAT".into());
        chat.badge = Some("6".into());
        chat.badge_style = BadgeStyle::Count;
        let mut level = MenuItem::new_action("level", "Menu Level");
        level.icon = Some("grid-view".into());
        let mut sub = MenuItem::new_action("sub", "Salma");
        sub.icon = Some("user".into());
        sub.action = Some("open-form:SALMA".into());
        level.items.push(sub);
        vec![home, modern, analytical, apps, chat, level]
    }

    fn state<'a>(
        items: &'a [MenuItem],
        collapsed: bool,
        expanded: &'a [String],
    ) -> SidebarState<'a> {
        SidebarState {
            items,
            collapsed,
            chrome: SidebarChrome {
                app_title: "AdminMart".into(),
                header_h: HEADER_H,
                footer_h: FOOTER_H,
            },
            palette: SidebarPalette {
                fg: Color32::WHITE,
                dim: Color32::GRAY,
                accent: Color32::BLUE,
                on_accent: Color32::WHITE,
                hover: Color32::DARK_GRAY,
            },
            selected: Some("modern"),
            expanded,
            hovered: None,
            icon_style: crate::icons::IconStyle::tint(Color32::WHITE),
            icon_size: ICON,
            icon_size_collapsed: ICON,
            scroll: 0.0,
            bg: Color32::TRANSPARENT,
            backdrop: Color32::TRANSPARENT,
            gradient: None,
            header_image: None,
            header_icon: None,
            shadow: None,
            font: FontId::proportional(14.0),
        }
    }

    /// A labelled separator is a section header; an unlabelled one is a rule.
    /// Rows never escape the rail — the bug the operator photographed.
    #[test]
    fn layout_places_sections_rows_and_profile_inside_the_rail() {
        let items = sample();
        let no_expand: Vec<String> = Vec::new();
        let st = state(&items, false, &no_expand);
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(OPEN_WIDTH, 700.0));
        let rows = layout(rect, &st);

        let sections: Vec<&str> = rows
            .iter()
            .filter_map(|r| match &r.kind {
                RowKind::Section(t) if !t.is_empty() => Some(t.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(sections, vec!["Home", "Apps"], "labelled separators head sections");

        let items_rows = rows
            .iter()
            .filter(|r| matches!(r.kind, RowKind::Item { .. }))
            .count();
        assert_eq!(items_rows, 4, "4 action rows (collapsed child hidden)");

        assert!(matches!(rows.first().unwrap().kind, RowKind::Header));
        assert!(matches!(rows.last().unwrap().kind, RowKind::Footer));
        for r in &rows {
            assert!(
                rect.contains_rect(r.rect),
                "{:?} escaped the rail: {:?}",
                r.kind,
                r.rect
            );
        }
        eprintln!(
            "049 sidebar layout — {} rows inside a {}x{} rail: header pane + 2 \
             sections + 4 items + footer pane, none escaping",
            rows.len(),
            OPEN_WIDTH,
            700.0
        );
    }

    /// The three panes tile the rail exactly: header on top, footer anchored
    /// to the bottom at full width, menu taking what is left. Collapsing hides
    /// the footer and hands its height to the icons.
    #[test]
    fn three_panes_tile_the_rail_and_the_footer_hides_on_the_rail() {
        let items = sample();
        let none: Vec<String> = Vec::new();
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(OPEN_WIDTH, 700.0));

        let st = state(&items, false, &none);
        let rows = layout(rect, &st);
        let header = rows.iter().find(|r| r.kind == RowKind::Header).unwrap();
        let footer = rows.iter().find(|r| r.kind == RowKind::Footer).unwrap();
        assert_eq!(header.rect.min.y, rect.min.y, "header sits at the top");
        assert_eq!(footer.rect.max.y, rect.max.y, "footer is anchored to the bottom");
        for pane in [header, footer] {
            assert_eq!(pane.rect.width(), rect.width(), "panes span the full width");
            assert_eq!(pane.rect.min.x, rect.min.x);
        }
        // The menu pane is the gap between them, and every item lands in it.
        let (top, bottom) = (header.rect.max.y, footer.rect.min.y);
        for r in rows.iter().filter(|r| matches!(r.kind, RowKind::Item { .. })) {
            assert!(
                r.rect.min.y >= top && r.rect.max.y <= bottom,
                "item {:?} escaped the menu pane",
                r.kind
            );
        }

        // Collapsed: the panes keep the heights the developer set — both of
        // them. The rail used to drop the footer and hand its height to the
        // icons, which moved all three panes under the operator every time
        // they collapsed it.
        let col = state(&items, true, &none);
        let col_rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(COLLAPSED_WIDTH, 700.0));
        let rows = layout(col_rect, &col);
        let col_footer = rows
            .iter()
            .find(|r| r.kind == RowKind::Footer)
            .expect("the collapsed rail keeps its footer pane");
        assert_eq!(col_footer.rect.height(), footer.rect.height());
        assert_eq!(col_footer.rect.max.y, col_rect.max.y);
        let col_header = rows
            .iter()
            .find(|r| r.kind == RowKind::Header)
            .expect("…and its header pane");
        assert_eq!(col_header.rect.height(), header.rect.height());

        eprintln!(
            "049 sidebar panes — header {:.0}px at top, footer {:.0}px at bottom, \
             both full width; menu pane = the {:.0}px between; collapsed drops \
             the footer (0 rows)",
            header.rect.height(),
            footer.rect.height(),
            bottom - top
        );
    }

    /// Header and footer heights come from the control's properties.
    #[test]
    fn pane_heights_follow_the_control_properties() {
        let mut ctrl = Control::new("SIDE", crate::ControlType::SideMenu, 0, 0);
        ctrl.set_prop("HeaderHeight", 90i64);
        ctrl.set_prop("FooterHeight", 120i64);
        let chrome = SidebarChrome::from_control(&ctrl);
        assert_eq!(chrome.header_h, 90.0);
        assert_eq!(chrome.footer_h, 120.0);

        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(OPEN_WIDTH, 700.0));
        let f = footer_rect(rect, &chrome, false).expect("open rail has a footer");
        assert_eq!(f.height(), 120.0);
        assert_eq!(f.max.y, rect.max.y);
        assert_eq!(f.width(), rect.width());
        assert_eq!(
            footer_rect(rect, &chrome, true),
            Some(f),
            "a collapsed rail lays the Panel into the SAME band — the heights \
             are the developer's in both states"
        );
        let mut no_footer = chrome.clone();
        no_footer.footer_h = 0.0;
        assert!(
            footer_rect(rect, &no_footer, false).is_none(),
            "a footer of zero height is the one case with no band"
        );
    }

    /// A rail shown collapsed is DRAWN narrow — the rule every surface shares.
    /// The preview used to skip this and paint collapsed rows across the rail's
    /// DESIGNED width, so the bar looked open while behaving closed and the
    /// breadcrumb beside it landed inside the bar.
    #[test]
    fn a_collapsed_rail_is_drawn_at_the_collapsed_width_everywhere() {
        use crate::model::{ControlType, Rect as MRect};

        let mut side = Control::new("SideMenu-1", ControlType::SideMenu, 0, 0);
        side.rect = MRect::new(0, 0, 280, 920);
        let mut footer = Control::new(
            &crate::model::side_menu_footer_id("SideMenu-1"),
            ControlType::Panel,
            0,
            860,
        );
        footer.rect = MRect::new(0, 860, 280, 60);
        let mut other = Control::new("Label-1", ControlType::Label, 400, 40);
        other.rect = MRect::new(400, 40, 200, 30);
        let controls = vec![side.clone(), footer, other];

        let collapsed_w = shown_width(&side, true) as i32;
        assert!(
            collapsed_w < 280,
            "the collapsed rail must be narrower than the design ({collapsed_w})"
        );

        let view = rail_view(&controls, &side, true);
        assert_eq!(view[0].rect.w, collapsed_w, "the rail narrows");
        assert_eq!(view[1].rect.w, collapsed_w, "its footer narrows with it");
        assert_eq!(view[2].rect.w, 200, "every other control is untouched");
        assert!(
            view[0].get_prop("Collapsed").map(|v| v.as_bool()).unwrap_or(false),
            "the live collapsed state rides on the painted control"
        );

        // Shown OPEN, the rail keeps its designed width.
        let open = rail_view(&controls, &side, false);
        assert_eq!(open[0].rect.w, 280);
        assert_eq!(open[1].rect.w, 280);

        // The DESIGN is never touched — this list is for painting only.
        assert_eq!(controls[0].rect.w, 280, "the designed rect is left alone");
    }

    /// Expanding a parent reveals its children as indented rows.
    #[test]
    fn expanding_a_parent_adds_its_children() {
        let items = sample();
        let none: Vec<String> = Vec::new();
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(OPEN_WIDTH, 700.0));
        let closed = layout(rect, &state(&items, false, &none)).len();
        let open_list = vec!["level".to_string()];
        let opened = layout(rect, &state(&items, false, &open_list));
        assert_eq!(opened.len(), closed + 1, "one child row appears");
        let child = opened
            .iter()
            .find(|r| matches!(&r.kind, RowKind::Item { id, .. } if id == "sub"))
            .expect("the child row is laid out");
        assert!(
            matches!(&child.kind, RowKind::Item { depth, .. } if *depth == 1),
            "the child is indented one level"
        );
    }

    /// The collapsed rail carries what can be reached BY AN ICON, and nothing
    /// else (operator, 2026-08-17).
    ///
    /// It used to carry every top-level item, which put groups on it — a row
    /// that opens a list with nowhere to open it to — while hiding the very
    /// items inside them that a shortcut rail exists for.
    #[test]
    fn the_collapsed_rail_carries_only_what_an_icon_can_reach() {
        let mut items = sample();
        // An item that does something but has no icon: nothing to draw and
        // nothing to aim at.
        let mut unlabelled = MenuItem::new_action("no-icon", "Reports");
        unlabelled.action = Some("open-form:REPORTS".into());
        items.push(unlabelled);
        // An icon that does nothing: a label wearing a picture.
        let mut inert = MenuItem::new_action("no-action", "Section");
        inert.icon = Some("folder".into());
        items.push(inert);

        let none: Vec<String> = Vec::new();
        let st = state(&items, true, &none);
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(COLLAPSED_WIDTH, 700.0));
        let rows = layout(rect, &st);
        let shown: Vec<&str> = rows
            .iter()
            .filter_map(|r| match &r.kind {
                RowKind::Item { id, .. } => Some(id.as_str()),
                _ => None,
            })
            .collect();

        assert_eq!(
            shown,
            vec!["modern", "analytical", "chat", "sub"],
            "the rail is icons with actions — `level` is a group, `no-icon` has \
             nothing to draw, `no-action` does nothing, and `sub` comes up out of \
             the group that no longer appears"
        );

        // A surfaced child keeps its TRUE path, or the shell could not tell which
        // menu slot the click belongs to, nor find the item again.
        let sub_path = rows.iter().find_map(|r| match &r.kind {
            RowKind::Item { id, path, depth } if id == "sub" => Some((path.clone(), *depth)),
            _ => None,
        });
        let (path, depth) = sub_path.expect("the child is on the rail");
        assert_eq!(path, vec![5, 0], "the path still walks the real tree");
        assert_eq!(item_at(&items, &path).map(|i| i.label.as_str()), Some("Salma"));
        assert_eq!(depth, 0, "a one-column rail does not indent");

        // Dividers survive only between two icons: "Home" led the list with
        // nothing above it, "Apps" sits between two icons and stays.
        let dividers: Vec<&str> = rows
            .iter()
            .filter_map(|r| match &r.kind {
                RowKind::Section(t) => Some(t.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(dividers, vec!["Apps"], "a divider needs an icon on both sides");

        for r in &rows {
            assert!(rect.contains_rect(r.rect), "{:?} escaped the rail", r.kind);
        }

        println!(
            "\n  Collapsed rail — 6 top-level entries + 2 edge cases reduce to 4 icons \
             (modern, analytical, chat, and `sub` surfaced from the group above it); the \
             group, the icon-less action and the action-less icon are all dropped; the \
             surfaced child keeps path [5,0] so its slot and its item still resolve; 1 of \
             2 section dividers survives\n"
        );
    }

    /// A group's children are INDENTED — the icon and the label together
    /// (operator, 2026-08-17).
    ///
    /// The row moves as one unit. It briefly held a single shared label column
    /// with only the icon stepping in, which left the icon adrift between two
    /// columns and gave the expanded menu no indentation to read at all.
    #[test]
    fn a_groups_children_are_indented_icon_and_label_together() {
        let items = sample();
        let expanded = vec!["level".to_string()];
        let st = state(&items, false, &expanded);
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(260.0, 700.0));
        let rows = layout(rect, &st);

        // Paint for real and read back where each label landed.
        let ctx = ctx();
        ctx.set_fonts(egui::FontDefinitions::default());
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            paint(ui.painter(), rect, &rows, &st);
        });
        out.textures_delta.clear();
        let mut label_x: Vec<(String, f32)> = Vec::new();
        for cs in &out.shapes {
            if let egui::Shape::Text(t) = &cs.shape {
                let text = t.galley.text().to_owned();
                if !text.trim().is_empty() {
                    label_x.push((text, t.pos.x));
                }
            }
        }
        let x_of = |needle: &str| -> f32 {
            label_x
                .iter()
                .find(|(t, _)| t == needle)
                .unwrap_or_else(|| panic!("'{needle}' was not painted: {label_x:?}"))
                .1
        };

        let group = x_of("Menu Level");
        let child = x_of("Salma");
        assert!(
            (child - group - NEST_INDENT).abs() < 0.01,
            "a child's name is set in one level from its group's: group {group}, \
             child {child}, expected {}",
            group + NEST_INDENT
        );
        // Every top-level row still shares one edge — only nesting moves a row.
        assert!((x_of("Modern") - group).abs() < 0.01);
        assert!((x_of("Chat") - group).abs() < 0.01);

        // The ICON takes the same step, so it keeps its own distance from its
        // own label at every depth — the row moves as one.
        let row = Rect::from_min_size(Pos2::ZERO, Vec2::new(260.0, ROW_H));
        let parent_icon = item_icon_rect(row, 0, ICON, false);
        let child_icon = item_icon_rect(row, 1, ICON, false);
        assert!(
            (child_icon.min.x - parent_icon.min.x - NEST_INDENT).abs() < 0.01,
            "the icon is indented with the label: {} vs {}",
            parent_icon.min.x,
            child_icon.min.x
        );
        assert_eq!(parent_icon.size(), child_icon.size(), "indenting is not resizing");
        let gap = |d: usize| item_label_x(row, d, ICON) - item_icon_rect(row, d, ICON, false).max.x;
        assert!(
            (gap(0) - gap(1)).abs() < 0.01,
            "icon-to-label gap must not change with depth: {} vs {}",
            gap(0),
            gap(1)
        );

        println!(
            "\n  Sidebar indentation — top-level 'Menu Level', 'Modern' and 'Chat' start at \
             x={group:.1}; the child 'Salma' at x={child:.1}, one {NEST_INDENT:.0}px level in, \
             with its icon stepped by the same {NEST_INDENT:.0}px and the {:.0}px icon-to-label \
             gap unchanged at both depths\n",
            gap(0)
        );
    }

    /// Each rail state draws its icons at its OWN size (operator,
    /// 2026-08-17): `IconSize` open, `IconSizeCollapsed` on the rail.
    ///
    /// One size could not serve both — beside a label an icon must not
    /// overpower the text, and alone on a rail that same size is lost.
    #[test]
    fn each_rail_state_draws_icons_at_its_own_size() {
        let ctx = ctx();
        let items = sample();
        let none: Vec<String> = Vec::new();

        let mut ctrl = Control::new("SIDE", crate::ControlType::SideMenu, 0, 0);
        ctrl.set_prop("IconSize", 20i64);
        ctrl.set_prop("IconSizeCollapsed", 34i64);
        let st = state_for_control(&ctx, &ctrl, &items, 255, &none);
        assert_eq!((st.icon_size, st.icon_size_collapsed), (20.0, 34.0));

        // BOTH ride on the state, and `collapsed` picks: a surface that flips
        // the rail after building the state still draws the matching size.
        let row = Rect::from_min_size(Pos2::ZERO, Vec2::new(COLLAPSED_WIDTH, ROW_H));
        let drawn = |collapsed: bool| {
            let size = if collapsed { st.icon_size_collapsed } else { st.icon_size };
            item_icon_rect(row, 0, size, collapsed).width()
        };
        assert_eq!(drawn(false), 20.0);
        assert_eq!(drawn(true), 34.0);

        // A form saved before the collapsed size existed keeps the look it was
        // designed with: the open size serves both.
        let mut old = Control::new("OLD", crate::ControlType::SideMenu, 0, 0);
        old.set_prop("IconSize", 26i64);
        old.properties.shift_remove("IconSizeCollapsed");
        let st = state_for_control(&ctx, &old, &items, 255, &none);
        assert_eq!(
            (st.icon_size, st.icon_size_collapsed),
            (26.0, 26.0),
            "an absent collapsed size falls back to the open one, not to a constant"
        );

        // Neither set → the built-in size, and a value too small to draw is
        // refused rather than painted as a speck.
        let bare = Control::new("BARE", crate::ControlType::SideMenu, 0, 0);
        let mut bare = bare.clone();
        bare.properties.shift_remove("IconSize");
        bare.properties.shift_remove("IconSizeCollapsed");
        let st = state_for_control(&ctx, &bare, &items, 255, &none);
        assert_eq!((st.icon_size, st.icon_size_collapsed), (ICON, ICON));
        let mut tiny = Control::new("TINY", crate::ControlType::SideMenu, 0, 0);
        tiny.set_prop("IconSize", 30i64);
        tiny.set_prop("IconSizeCollapsed", 0i64);
        let st = state_for_control(&ctx, &tiny, &items, 255, &none);
        assert_eq!(st.icon_size_collapsed, 30.0, "0 is not a size; the open one stands in");

        // The DEFAULT for a new SideMenu is the same in both states, so adding
        // the property restyles nothing until the developer chooses to.
        let fresh = Control::new("NEW", crate::ControlType::SideMenu, 0, 0);
        assert_eq!(
            fresh.get_prop("IconSize").map(|v| v.as_i64()),
            fresh.get_prop("IconSizeCollapsed").map(|v| v.as_i64())
        );

        println!(
            "\n  Sidebar icon sizes — IconSize 20pt open / IconSizeCollapsed 34pt on the rail, \
             both carried on the state so the painter picks by rail state; an absent or \
             unusable collapsed size falls back to the open one (26→26, 0→30); neither set \
             gives {ICON:.0}pt; a new SideMenu defaults both to the same value\n"
        );
    }

    /// HOME stands apart on the collapsed rail (operator, 2026-08-17): the
    /// distance from it to the icon below is twice the distance between any
    /// other two — and it is found by its ACTION, never by its label.
    #[test]
    fn home_keeps_twice_the_distance_from_the_icon_below_it() {
        let none: Vec<String> = Vec::new();
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(COLLAPSED_WIDTH, 900.0));
        let centres = |items: &[MenuItem]| -> Vec<(String, f32)> {
            layout(rect, &state(items, true, &none))
                .iter()
                .filter_map(|r| match &r.kind {
                    RowKind::Item { id, .. } => Some((id.clone(), r.rect.center().y)),
                    _ => None,
                })
                .collect()
        };

        // `modern` is the fixture's Home: action "home", and it is followed on
        // the rail by `analytical`.
        let items = sample();
        let ys = centres(&items);
        let y_of = |id: &str| -> f32 {
            ys.iter().find(|(i, _)| i == id).unwrap_or_else(|| panic!("{id} is not on the rail: {ys:?}")).1
        };
        let regular = ROW_H + ROW_GAP;
        assert!(
            ((y_of("analytical") - y_of("modern")) - regular * 2.0).abs() < 0.01,
            "Home to the icon below it must be twice {regular}: {}",
            y_of("analytical") - y_of("modern")
        );
        assert!(
            ((y_of("sub") - y_of("chat")) - regular).abs() < 0.01,
            "every other pair keeps the regular distance: {}",
            y_of("sub") - y_of("chat")
        );

        // The LABEL decides nothing. Calling something "Home" earns no space…
        let mut renamed = sample();
        renamed[1].action = Some("open-form:START".into());
        renamed[1].label = "Home".into();
        renamed[2].label = "Not Home".into();
        let ys = centres(&renamed);
        let y2 = |id: &str| ys.iter().find(|(i, _)| i == id).unwrap().1;
        assert!(
            ((y2("analytical") - y2("modern")) - regular).abs() < 0.01,
            "a row merely CALLED Home is an ordinary icon"
        );

        // …and the action alone moves the space to whichever row carries it.
        let mut moved = sample();
        moved[1].action = Some("open-form:START".into());
        moved[4].action = Some("home".into()); // `chat`, with `sub` below it
        let ys = centres(&moved);
        let y3 = |id: &str| ys.iter().find(|(i, _)| i == id).unwrap().1;
        assert!(
            ((y3("analytical") - y3("modern")) - regular).abs() < 0.01,
            "the row that lost the action lost the space"
        );
        assert!(
            ((y3("sub") - y3("chat")) - regular * 2.0).abs() < 0.01,
            "the row that gained it gained the space: {}",
            y3("sub") - y3("chat")
        );
        assert!(is_home(&moved[4]) && !is_home(&moved[1]));

        // A Home with a DIVIDER under it is already separated, so nothing is
        // added: the distance is the regular one plus the divider itself.
        let mut before_divider = sample();
        before_divider[1].action = Some("open-form:START".into());
        before_divider[2].action = Some("home".into()); // `analytical`, then "Apps"
        let ys = centres(&before_divider);
        let y4 = |id: &str| ys.iter().find(|(i, _)| i == id).unwrap().1;
        assert!(
            ((y4("chat") - y4("analytical")) - (regular + SECTION_H)).abs() < 0.01,
            "a divider separates on its own: {}",
            y4("chat") - y4("analytical")
        );

        println!(
            "\n  Collapsed rail spacing — icons sit {regular:.0}px apart; Home ('modern', \
             action \"home\") sits {:.0}px above the next icon, exactly twice; renaming a \
             row 'Home' changes nothing and moving the ACTION moves the space with it\n",
            y_of("analytical") - y_of("modern")
        );
    }

    /// The rule itself, item by item.
    #[test]
    fn shows_on_rail_wants_an_icon_an_action_and_no_children() {
        let mut good = MenuItem::new_action("a", "Good");
        good.icon = Some("home".into());
        good.action = Some("home".into());
        assert!(shows_on_rail(&good));

        let mut no_icon = good.clone();
        no_icon.icon = None;
        assert!(!shows_on_rail(&no_icon), "nothing to draw");
        let mut blank_icon = good.clone();
        blank_icon.icon = Some("   ".into());
        assert!(!shows_on_rail(&blank_icon), "whitespace is not an icon");

        let mut no_action = good.clone();
        no_action.action = None;
        assert!(!shows_on_rail(&no_action), "nothing to do");
        let mut blank_action = good.clone();
        blank_action.action = Some(String::new());
        assert!(!shows_on_rail(&blank_action));

        let mut group = good.clone();
        group.items.push(MenuItem::new_action("kid", "Kid"));
        assert!(!shows_on_rail(&group), "a group opens a list; a rail has no room");

        let mut separator = MenuItem::new_separator("s");
        separator.icon = Some("home".into());
        separator.action = Some("home".into());
        assert!(!shows_on_rail(&separator), "a separator is not an element");

        println!(
            "\n  Rail rule — an item needs an icon AND an action AND no children; a \
             missing or blank icon, a missing or blank action, a group and a separator \
             are each refused\n"
        );
    }

    /// A menu taller than its pane SCROLLS — it does not silently lose its
    /// tail, which is what the operator photographed. The header and footer
    /// panes stay put, so the toggle is reachable at any scroll offset.
    #[test]
    fn a_menu_taller_than_its_pane_scrolls_instead_of_being_clipped() {
        // Enough items that they cannot all fit a 300px rail.
        let items: Vec<MenuItem> = (0..20)
            .map(|i| MenuItem::new_action(&format!("i{i}"), &format!("Item {i}")))
            .collect();
        let none: Vec<String> = Vec::new();
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(OPEN_WIDTH, 300.0));

        let mut st = state(&items, false, &none);
        let band = menu_band(rect, &st);
        let content = menu_content_height(rect, &st);
        assert!(
            content > band.height(),
            "the fixture must actually overflow: {content} vs {}",
            band.height()
        );
        let max = max_scroll(rect, &st);
        assert!(max > 0.0, "an overflowing menu can scroll");

        let ids = |rows: &[SidebarRow]| -> Vec<String> {
            rows.iter()
                .filter_map(|r| match &r.kind {
                    RowKind::Item { id, .. } => Some(id.clone()),
                    _ => None,
                })
                .collect()
        };

        let top = layout(rect, &st);
        let top_ids = ids(&top);
        assert!(top_ids.contains(&"i0".to_string()), "the first row is at rest");
        assert!(
            !top_ids.contains(&"i19".to_string()),
            "the last row is past the pane — that is what scrolling is for"
        );

        // Scrolled to the end, the tail is reachable and the head is gone.
        st.scroll = max;
        let bottom = layout(rect, &st);
        let bottom_ids = ids(&bottom);
        assert!(
            bottom_ids.contains(&"i19".to_string()),
            "the last row is reachable: {bottom_ids:?}"
        );
        assert!(!bottom_ids.contains(&"i0".to_string()), "the head scrolled away");

        // The panes themselves never scroll, and no menu row is ever clickable
        // outside the band.
        for rows in [&top, &bottom] {
            let header = rows.iter().find(|r| r.kind == RowKind::Header).unwrap();
            let footer = rows.iter().find(|r| r.kind == RowKind::Footer).unwrap();
            assert_eq!(header.rect.min.y, rect.min.y, "the header pane is fixed");
            assert_eq!(footer.rect.max.y, rect.max.y, "the footer pane is fixed");
            for r in rows.iter().filter(|r| !matches!(r.kind, RowKind::Header | RowKind::Footer)) {
                assert!(
                    band.contains_rect(r.visible),
                    "{:?} is clickable outside the menu pane: {:?}",
                    r.kind,
                    r.visible
                );
            }
        }

        // Past the end it clamps rather than running off into space.
        st.scroll = max + 5_000.0;
        assert_eq!(clamp_scroll(rect, &st), max, "scroll clamps at the end");
        st.scroll = -500.0;
        assert_eq!(clamp_scroll(rect, &st), 0.0, "and at the start");

        eprintln!(
            "049 sidebar scroll — 20 items = {content:.0}px of rows in a \
             {:.0}px menu pane; scroll range 0..{max:.0}px; header/footer \
             fixed; every visible row confined to the pane",
            band.height()
        );
    }

    /// The rail wears the background the developer set in the RAD, resolved
    /// against the form behind it — the same value on every surface.
    ///
    /// The operator's own form designs `#F6F6F639`: white at 22 %. That is
    /// meant to read as a lift off the form's navy, and it did on the canvas
    /// only because the generic control frame happened to paint it there. The
    /// preview painted no background at all and the shell composed its own, so
    /// one design produced three rails.
    #[test]
    fn the_rail_wears_the_background_defined_in_the_rad() {
        let ctx = ctx();
        let items = sample();
        let none: Vec<String> = Vec::new();
        let mut ctrl = Control::new("SIDE", crate::ControlType::SideMenu, 0, 0);
        ctrl.set_prop("BackgroundColor", "#F6F6F639");

        let st = state_for_control(&ctx, &ctrl, &items, 255, &none);
        let designed = crate::paint::parse_color("#F6F6F639");
        assert_eq!(st.bg, designed, "the designed colour reaches the painter intact");
        assert!(st.bg.a() > 0 && st.bg.a() < 255, "…translucent, as designed");

        // Over the form's navy it is a lift off the backdrop, nowhere near
        // white — the shade the operator sees on the canvas.
        let navy = crate::render::backdrop_color("00000000", 0);
        let resolved = crate::paint::composite_premultiplied_over(st.bg, navy);
        assert_eq!(resolved.a(), 255, "opaque over an opaque form");
        for (ch, base) in [
            (resolved.r(), navy.r()),
            (resolved.g(), navy.g()),
            (resolved.b(), navy.b()),
        ] {
            assert!(ch > base, "the rail lifts off the form: {resolved:?}");
            assert!(ch < 128, "and never washes out to white: {resolved:?}");
        }

        // Fading the whole control fades its rail with it, and a control with
        // no BackgroundColor contributes nothing rather than a black rectangle.
        let faded = state_for_control(&ctx, &ctrl, &items, 128, &none);
        assert!(faded.bg.a() < st.bg.a(), "alpha_mul fades the rail");
        let bare = Control::new("SIDE2", crate::ControlType::SideMenu, 0, 0);
        let bare_st = state_for_control(&ctx, &bare, &items, 255, &none);
        assert_eq!(
            bare_st.bg,
            Color32::TRANSPARENT,
            "no BackgroundColor paints no background"
        );

        eprintln!(
            "049 sidebar background — designed #F6F6F639 → {:?}; over the \
             form's {navy:?} → {resolved:?} (opaque, still dark)",
            st.bg
        );
    }

    /// `HeaderImage` resolves to a texture in the ONE place every surface
    /// builds its state, so setting it cannot be honoured on one surface and
    /// ignored on the rest — which is what happened while each surface was
    /// expected to load the image and hand it in, and none of them did.
    #[test]
    fn a_header_image_resolves_for_every_surface_at_once() {
        let ctx = ctx();
        let items = sample();
        let none: Vec<String> = Vec::new();

        let mut ctrl = Control::new("SIDE", crate::ControlType::SideMenu, 0, 0);
        assert!(
            state_for_control(&ctx, &ctrl, &items, 255, &none)
                .header_image
                .is_none(),
            "no HeaderImage, no logo — and no placeholder mark either"
        );

        // A real 2x2 PNG, written where the loader will actually find it.
        let dir = std::env::temp_dir().join("cobolt-049-header-image");
        std::fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join("logo.png");
        let img = image::RgbaImage::from_pixel(2, 2, image::Rgba([10, 200, 90, 255]));
        img.save(&path).expect("write png");

        ctrl.set_prop("HeaderImage", path.to_string_lossy().as_ref());
        let st = state_for_control(&ctx, &ctrl, &items, 255, &none);
        assert!(
            st.header_image.is_some(),
            "the property resolves to a texture the painter can draw"
        );

        // A path that does not resolve degrades to no logo, never a panic.
        ctrl.set_prop("HeaderImage", "/nowhere/at/all/logo.png");
        assert!(
            state_for_control(&ctx, &ctrl, &items, 255, &none)
                .header_image
                .is_none(),
            "an unresolvable path draws nothing"
        );

        let _ = std::fs::remove_dir_all(&dir);
        eprintln!(
            "049 sidebar HeaderImage — unset → None, real file → texture, \
             missing file → None (no panic)"
        );
    }

    /// The logo box is the LARGEST a logo is drawn at when the rail is open --
    /// 270x80 (operator, 2026-08-18). It shrinks, keeping its shape, only when
    /// the rail or the header cannot hold it; and the image inside it is drawn
    /// at its own size up to the box, scaled down to fit when it is bigger,
    /// rather than stretched to fill it.
    #[test]
    fn the_header_logo_box_is_a_270x80_limit_the_image_fits_inside() {
        // A rail wide and tall enough: the box is exactly the limit.
        let header = Rect::from_min_size(Pos2::ZERO, Vec2::new(320.0, 100.0));
        let r = header_image_rect(header, false);
        assert_eq!((r.width(), r.height()), (HEADER_IMAGE_W, HEADER_IMAGE_H));
        assert_eq!((r.width(), r.height()), (270.0, 80.0), "the stated limit");
        // Centred in the pane, both axes and in BOTH rail states. It used to be
        // pinned to the left padding when open, which left the logo off-centre
        // against the pane it is the whole content of (operator, 1.61.38).
        assert!(
            (r.center().x - header.center().x).abs() < 0.01,
            "horizontally centred when open: {} vs {}",
            r.center().x,
            header.center().x
        );
        assert!(
            (r.center().y - header.center().y).abs() < 0.01,
            "and vertically centred"
        );
        assert!(header.contains_rect(r), "and inside the header pane");

        // The SideMenu's own default header height must hold the box, or the
        // limit is one a developer can never actually reach.
        let default_header = Rect::from_min_size(Pos2::ZERO, Vec2::new(320.0, 120.0));
        let r_def = header_image_rect(default_header, false);
        assert_eq!(
            (r_def.width(), r_def.height()),
            (HEADER_IMAGE_W, HEADER_IMAGE_H),
            "the seeded 120pt header must hold the full 270x80 box"
        );

        let aspect = HEADER_IMAGE_W / HEADER_IMAGE_H;
        // The collapsed rail is a fraction of the box's width: it shrinks,
        // centred, and keeps its shape rather than squashing the logo.
        let rail = Rect::from_min_size(Pos2::ZERO, Vec2::new(COLLAPSED_WIDTH, 64.0));
        let r = header_image_rect(rail, true);
        assert!(r.width() < HEADER_IMAGE_W, "shrunk to the rail");
        assert!((r.width() / r.height() - aspect).abs() < 0.01, "shape kept");
        assert!(
            (r.center().x - rail.center().x).abs() < 0.5,
            "centred on the collapsed rail"
        );
        assert!(rail.contains_rect(r));

        // A header SHORTER than the box shrinks it too -- height binds first.
        let short = Rect::from_min_size(Pos2::ZERO, Vec2::new(320.0, 40.0));
        let r = header_image_rect(short, false);
        assert!(r.height() <= short.height() - 8.0, "fits the short header");
        assert!((r.width() / r.height() - aspect).abs() < 0.01, "shape kept");
        assert!(short.contains_rect(r));

        // -- The image inside the box: a limit, not a shape to fill ---------
        //
        // This is the rule the operator asked for: up to 270x80 the logo is
        // drawn as it is; beyond it, scaled down to fit. `paint_header` routes
        // the image through the same helper, with the box as the bound.
        let box_rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(270.0, 80.0));
        let fit = |w: f32, h: f32| {
            crate::paint::media_dest_rect(box_rect, Vec2::new(w, h), "Center")
        };

        // Smaller than the limit: its own size, untouched and centred.
        let small = fit(120.0, 40.0);
        assert_eq!(
            (small.width(), small.height()),
            (120.0, 40.0),
            "a logo inside the limit keeps its own size, not stretched to fill"
        );
        assert!(
            (small.center() - box_rect.center()).length() < 0.01,
            "and is centred in the box"
        );

        // Exactly the limit: unchanged.
        assert_eq!((fit(270.0, 80.0).width(), fit(270.0, 80.0).height()), (270.0, 80.0));

        // Wider than the limit: width binds, aspect held.
        let wide = fit(540.0, 80.0);
        assert!(wide.width() <= 270.0 + 0.01, "scaled down to the width limit");
        assert!(
            (wide.width() / wide.height() - 540.0 / 80.0).abs() < 0.01,
            "aspect ratio held: {}x{}",
            wide.width(),
            wide.height()
        );

        // Taller than the limit: height binds, aspect held.
        let tall = fit(270.0, 240.0);
        assert!(tall.height() <= 80.0 + 0.01, "scaled down to the height limit");
        assert!(
            (tall.width() / tall.height() - 270.0 / 240.0).abs() < 0.01,
            "aspect ratio held: {}x{}",
            tall.width(),
            tall.height()
        );

        // A big square: fits inside on both axes, still square.
        let square = fit(1000.0, 1000.0);
        assert!(square.width() <= 270.0 + 0.01 && square.height() <= 80.0 + 0.01);
        assert!(
            (square.width() - square.height()).abs() < 0.01,
            "a square logo stays square: {}x{}",
            square.width(),
            square.height()
        );

        eprintln!(
            "sidebar header logo -- box is a {:.0}x{:.0} LIMIT: 320x100 rail and the \
             seeded 120pt header both give the full box; {:.0}px collapsed rail shrinks \
             it keeping aspect {aspect:.2}; a 40pt header shrinks it too. Image inside: \
             120x40 stays 120x40, 540x80 -> {:.0}x{:.0}, 270x240 -> {:.0}x{:.0}, \
             1000x1000 -> {:.0}x{:.0} (square kept)",
            HEADER_IMAGE_W,
            HEADER_IMAGE_H,
            COLLAPSED_WIDTH,
            wide.width(),
            wide.height(),
            tall.width(),
            tall.height(),
            square.width(),
            square.height()
        );
    }

    /// The footer Panel's own Background and Border are the DEVELOPER'S.
    ///
    /// The rail owns where the Panel sits and nothing else — `paint_footer` is
    /// deliberately empty. Operator report: its background and border style
    /// could not be changed.
    /// **What belongs to the footer, by ancestry.** The shell has to hand the
    /// rail everything the developer put in the footer Panel, and a control two
    /// levels down is no less in the footer than a direct child — a Panel
    /// dropped into the footer takes its own contents with it.
    ///
    /// Nothing outside that subtree may be claimed: a control claimed here
    /// disappears from the ContentPane and reappears in a 144pt band.
    #[test]
    fn the_footer_subtree_is_everything_inside_it_and_nothing_else() {
        use crate::model::{side_menu_footer_subtree, Form, Rect as MRect};

        let mut form = Form::new("F", "F", 960, 744);
        let mut side = Control::new("SideMenu-1", crate::ControlType::SideMenu, 0, 0);
        side.rect = MRect::new(0, 0, 200, 744);
        side.set_prop("FooterHeight", 144i64);
        form.controls.push(side);
        form.sync_side_menu_footer_panels();

        let footer_id = crate::model::side_menu_footer_id("SideMenu-1");
        // A clock in the footer, a Panel in the footer, and a label in THAT
        // panel — listed child-before-parent on purpose, since a form's control
        // list is z-ordered and owes nobody a parents-first order.
        let mut deep = Control::new("LBL-DEEP", crate::ControlType::Label, 0, 0);
        deep.parent = Some("PNL-INNER".into());
        let mut inner = Control::new("PNL-INNER", crate::ControlType::Panel, 0, 0);
        inner.parent = Some(footer_id.clone());
        let mut clock = Control::new("LBL-CLOCK", crate::ControlType::Label, 0, 0);
        clock.parent = Some(footer_id.clone());
        // Content, and a control parented to the SIDEBAR itself rather than to
        // its footer — neither is the footer's.
        let content = Control::new("BTN-1", crate::ControlType::Button, 300, 40);
        let mut on_rail = Control::new("LBL-RAIL", crate::ControlType::Label, 10, 10);
        on_rail.parent = Some("SideMenu-1".into());
        form.controls.extend([deep, inner, clock, content, on_rail]);

        let mut ids = side_menu_footer_subtree(&form.controls);
        ids.sort();
        assert_eq!(
            ids,
            vec![
                "LBL-CLOCK".to_string(),
                "LBL-DEEP".to_string(),
                "PNL-INNER".to_string(),
                footer_id,
            ]
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>(),
            "the Panel, both its levels of children, and nothing else"
        );
    }

    #[test]
    fn the_footer_panel_takes_the_developers_background_and_border() {
        use crate::model::{Form, PropValue, Rect as MRect};

        let mut form = Form::new("F", "F", 960, 744);
        let mut side = Control::new("SideMenu-1", crate::ControlType::SideMenu, 0, 0);
        side.rect = MRect::new(0, 0, 200, 744);
        side.set_prop("FooterHeight", 72i64);
        form.controls.push(side);
        form.sync_side_menu_footer_panels();

        // The developer styles it like any other container.
        let f = form
            .controls
            .iter_mut()
            .find(|c| c.is_side_menu_footer())
            .expect("footer Panel");
        f.set_prop("BackgroundColor", PropValue::String("#C0392B".into()));
        f.set_prop("BorderStyle", PropValue::String("Single".into()));
        f.set_prop("BorderColor", PropValue::String("#F1C40F".into()));
        f.set_prop("BorderWidth", PropValue::Int(2));

        // Re-syncing must not undo any of it — the rail re-pins the RECT only.
        form.sync_side_menu_footer_panels();
        let f = form
            .controls
            .iter()
            .find(|c| c.is_side_menu_footer())
            .expect("footer Panel");
        assert_eq!(
            f.get_prop("BackgroundColor").map(|v| v.as_str().to_owned()),
            Some("#C0392B".to_owned()),
            "the rail overwrote the developer's background"
        );
        assert_eq!(
            f.get_prop("BorderStyle").map(|v| v.as_str().to_owned()),
            Some("Single".to_owned()),
            "the rail overwrote the developer's border style"
        );

        // And the painter honours them under a self-contained theme (R9).
        assert_eq!(
            crate::paint::user_background_color(f),
            Some(crate::paint::parse_color("#C0392B")),
            "an explicit background must outrank the theme's container colour"
        );

        let ctx = egui::Context::default();
        crate::paint::set_surface_theme(&ctx, crate::surface_theme::elegance());
        let rect = Rect::from_min_size(Pos2::new(0.0, 672.0), Vec2::new(200.0, 72.0));
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 800.0)));
        let mut full = ctx.run_ui(input, |ui| {
            crate::paint::draw_control(
                ui.painter(),
                Pos2::new(f.rect.x as f32, 0.0),
                f,
                false,
                true,
                1.0,
                1.0,
                None,
            );
        });
        full.textures_delta.clear();
        fn fills(s: &egui::Shape, out: &mut Vec<Color32>) {
            match s {
                egui::Shape::Vec(v) => v.iter().for_each(|s| fills(s, out)),
                egui::Shape::Rect(r) => {
                    out.push(r.fill);
                    out.push(r.stroke.color);
                }
                _ => {}
            }
        }
        let mut seen = Vec::new();
        for cs in &full.shapes {
            fills(&cs.shape, &mut seen);
        }
        let same = |a: Color32, b: Color32| a.r() == b.r() && a.g() == b.g() && a.b() == b.b();
        let want_bg = crate::paint::parse_color("#C0392B");
        let want_border = crate::paint::parse_color("#F1C40F");
        assert!(
            seen.iter().any(|c| same(*c, want_bg)),
            "the chosen background never reached the canvas; painted {:?}",
            seen.iter()
                .filter(|c| c.a() > 0)
                .map(|c| format!("#{:02x}{:02x}{:02x}", c.r(), c.g(), c.b()))
                .collect::<Vec<_>>()
        );
        assert!(
            seen.iter().any(|c| same(*c, want_border)),
            "the chosen border colour never reached the canvas"
        );
        let _ = rect;
        eprintln!(
            "049 footer Panel — Background #C0392B and Border Single/#F1C40F/2 \
             survive a re-sync and both reach the canvas under Elegance"
        );
    }

    /// The SideMenu's footer Panel is created once, pinned to the footer band,
    /// and is a real container — which is what makes it a drop target.
    #[test]
    fn the_footer_panel_is_created_once_and_pinned_to_the_band() {
        use crate::model::{Form, Rect as MRect};

        let mut form = Form::new("F", "F", 960, 744);
        let mut side = Control::new("SideMenu-1", crate::ControlType::SideMenu, 0, 0);
        side.rect = MRect::new(0, 0, 200, 744);
        side.set_prop("FooterHeight", 72i64);
        form.controls.push(side);

        form.sync_side_menu_footer_panels();
        let footers: Vec<&Control> =
            form.controls.iter().filter(|c| c.is_side_menu_footer()).collect();
        assert_eq!(footers.len(), 1, "exactly one footer Panel");
        let f = footers[0];
        assert_eq!(f.control_type, crate::ControlType::Panel);
        assert!(f.is_container(), "a container, so it can be dropped into");
        assert_eq!(f.parent.as_deref(), Some("SideMenu-1"), "owned by the rail");
        assert_eq!(
            (f.rect.x, f.rect.y, f.rect.w, f.rect.h),
            (0, 744 - 72, 200, 72),
            "pinned to the footer band"
        );

        // Idempotent: running it again neither duplicates nor moves it.
        form.sync_side_menu_footer_panels();
        assert_eq!(
            form.controls.iter().filter(|c| c.is_side_menu_footer()).count(),
            1,
            "re-syncing does not add a second Panel"
        );

        // It follows the rail rather than being dragged: shrink the form and
        // the band moves with it.
        form.controls[0].rect.h = 500;
        form.sync_side_menu_footer_panels();
        let f = form.controls.iter().find(|c| c.is_side_menu_footer()).unwrap();
        assert_eq!(f.rect.y, 500 - 72, "the Panel follows the rail's bottom edge");

        // Collapsing keeps the band: the Panel (and whatever the developer
        // dropped into it) must not vanish because the operator narrowed the
        // rail.
        form.controls[0].set_prop("Collapsed", true);
        form.sync_side_menu_footer_panels();
        let f = form.controls.iter().find(|c| c.is_side_menu_footer()).unwrap();
        assert_eq!(f.rect.h, 72, "a collapsed rail keeps its footer band");

        eprintln!(
            "049 footer Panel — one Panel, parent=SideMenu-1, pinned to \
             (0,{},200,72); idempotent; follows a rail resize; zero-height when \
             collapsed",
            744 - 72
        );
    }

    /// The header pane is exactly as tall as the developer said, on every
    /// surface and in both rail states — never recomputed, never trimmed to
    /// its contents, never scaled to the window.
    #[test]
    fn the_header_height_is_the_developers_and_is_never_resized() {
        assert_eq!(HEADER_H, 120.0, "the default the developer starts from");

        let mut ctrl = Control::new("SIDE", crate::ControlType::SideMenu, 0, 0);
        assert_eq!(
            SidebarChrome::from_control(&ctrl).header_h,
            HEADER_H,
            "a control that never set it gets the default"
        );
        ctrl.set_prop("HeaderHeight", 200i64);
        assert_eq!(
            SidebarChrome::from_control(&ctrl).header_h,
            200.0,
            "and what it DID set is used as given"
        );

        // Whatever the rail's size or state, the header keeps that height and
        // the menu band is simply what is left under it.
        let items = sample();
        let none: Vec<String> = Vec::new();
        for (w, h, collapsed) in [
            (OPEN_WIDTH, 700.0, false),
            (OPEN_WIDTH, 400.0, false),
            (COLLAPSED_WIDTH, 700.0, true),
        ] {
            let mut st = state(&items, collapsed, &none);
            st.chrome.header_h = 200.0;
            let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(w, h));
            let rows = layout(rect, &st);
            let header = rows.iter().find(|r| r.kind == RowKind::Header).unwrap();
            assert_eq!(
                header.rect.height(),
                200.0,
                "{w}x{h} collapsed={collapsed}: the header is not resized"
            );
            assert_eq!(header.rect.min.y, rect.min.y);
            assert_eq!(menu_band(rect, &st).min.y, rect.min.y + 200.0);
        }

        eprintln!(
            "049 sidebar header — default {HEADER_H:.0}px; a developer's 200px \
             held exactly across 240x700, 240x400 and the collapsed rail"
        );
    }

    /// The collapsed rail shows its ICON, not a shrunken logo — 45x45, centred
    /// on the rail and in the middle of a header pane that keeps its height.
    #[test]
    fn the_collapsed_rail_shows_the_header_icon_not_a_squeezed_logo() {
        let ctx = ctx();
        let items = sample();
        let none: Vec<String> = Vec::new();
        let mut ctrl = Control::new("SIDE", crate::ControlType::SideMenu, 0, 0);
        ctrl.set_prop("HeaderHeight", 120i64);

        // The mark is its OWN image file, picked from disk like the logo.
        let dir = std::env::temp_dir().join("cobolt-049-header-icon");
        std::fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join("mark.png");
        image::RgbaImage::from_pixel(2, 2, image::Rgba([10, 90, 200, 255]))
            .save(&path)
            .expect("write png");
        ctrl.set_prop("HeaderIcon", path.to_string_lossy().as_ref());
        assert!(
            state_for_control(&ctx, &ctrl, &items, 255, &none)
                .header_icon
                .is_some(),
            "the property resolves to a texture the painter can draw"
        );
        let _ = std::fs::remove_dir_all(&dir);

        // 45x45, centred both ways in the header pane of a collapsed rail.
        let rail = Rect::from_min_size(Pos2::ZERO, Vec2::new(COLLAPSED_WIDTH, 700.0));
        let mut col = state(&items, true, &none);
        col.chrome.header_h = 120.0;
        let rows = layout(rail, &col);
        let header = rows.iter().find(|r| r.kind == RowKind::Header).unwrap();
        assert_eq!(header.rect.height(), 120.0, "the header keeps its height");
        let mark = Rect::from_center_size(header.rect.center(), Vec2::splat(HEADER_ICON));
        assert_eq!(HEADER_ICON, 45.0);
        assert!((mark.center().x - rail.center().x).abs() < 0.5, "centred on the rail");
        assert!(
            (mark.center().y - header.rect.center().y).abs() < 0.5,
            "and in the middle of the header pane"
        );
        assert!(header.rect.contains_rect(mark), "it fits the pane it sits in");

        // Unset ⇒ no mark at all, rather than an invented one.
        let bare = Control::new("SIDE2", crate::ControlType::SideMenu, 0, 0);
        assert!(state_for_control(&ctx, &bare, &items, 255, &none)
            .header_icon
            .is_none());

        eprintln!(
            "049 collapsed header — 'home' drawn {HEADER_ICON:.0}x{HEADER_ICON:.0} \
             centred in a {:.0}px header on a {:.0}px rail; unset draws nothing",
            header.rect.height(),
            COLLAPSED_WIDTH
        );
    }

    /// A gradient REPLACES the rail's plain colour — it does not layer over it,
    /// and it is no longer ignored outright, which is what "background gradient
    /// is not working" meant: the painter only ever read `BackgroundColor`.
    #[test]
    fn an_enabled_gradient_replaces_the_rails_plain_colour() {
        let ctx = ctx();
        let items = sample();
        let none: Vec<String> = Vec::new();
        let mut ctrl = Control::new("SIDE", crate::ControlType::SideMenu, 0, 0);
        ctrl.set_prop("BackgroundColor", "#101010FF");

        // Off: no gradient, the plain colour stands.
        let st = state_for_control(&ctx, &ctrl, &items, 255, &none);
        assert!(st.gradient.is_none(), "no gradient unless the developer enables one");
        assert_eq!(st.bg, crate::paint::parse_color("#101010FF"));

        // On: the gradient's own colours and direction reach the painter.
        ctrl.set_prop("BackgroundGradientEnabled", true);
        ctrl.set_prop("BackgroundGradientStartColor", "#204080FF");
        ctrl.set_prop("BackgroundGradientEndColor", "#80C0FFFF");
        ctrl.set_prop("BackgroundGradientDirection", "East");
        let st = state_for_control(&ctx, &ctrl, &items, 255, &none);
        let (start, end, dir) = st.gradient.clone().expect("the gradient is read");
        assert_eq!(start, crate::paint::parse_color("#204080FF"));
        assert_eq!(end, crate::paint::parse_color("#80C0FFFF"));
        assert_eq!(dir, "East");

        // Fading the control fades the gradient with it.
        let faded = state_for_control(&ctx, &ctrl, &items, 128, &none);
        let (fs, _, _) = faded.gradient.clone().expect("still a gradient");
        assert!(fs.a() < start.a(), "alpha_mul fades the gradient too");

        // And it paints headlessly, over the form's backdrop.
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(OPEN_WIDTH, 400.0));
        let mut painted = state(&items, false, &none);
        painted.gradient = Some((start, end, "South".to_owned()));
        painted.backdrop = Color32::from_rgb(20, 22, 45);
        let rows = layout(rect, &painted);
        let mut full = ctx.run_ui(egui::RawInput::default(), |ui| {
            paint(ui.painter(), rect, &rows, &painted);
        });
        full.textures_delta.clear();

        eprintln!(
            "049 sidebar gradient — off ⇒ plain #101010FF; on ⇒ {start:?} → \
             {end:?} heading East, replacing the plain colour and fading with \
             the control"
        );
    }

    /// Hit-testing walks the SAME rectangles the painter drew, so a click can
    /// never land on a different row than the one under the pointer.
    #[test]
    fn hit_test_matches_the_painted_rows() {
        let items = sample();
        let none: Vec<String> = Vec::new();
        let st = state(&items, false, &none);
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(OPEN_WIDTH, 700.0));
        let rows = layout(rect, &st);
        let mut checked = 0;
        for (ix, row) in rows.iter().enumerate() {
            assert_eq!(row_at(&rows, row.rect.center()), Some(ix));
            checked += 1;
        }
        assert_eq!(row_at(&rows, Pos2::new(500.0, 500.0)), None, "outside hits nothing");
        eprintln!("049 sidebar hit-test — {checked}/{checked} row centres resolve to themselves");
    }

    /// The rail's drop shadow is the RAIL'S — resolved with the rest of the
    /// shared state and painted here, so it exists on the canvas, the preview,
    /// Run Form and the shell alike.
    ///
    /// It used to be left to the generic control frame, which a SideMenu skips
    /// (it owns its whole face); the shell and the preview never reach that
    /// code at all. The property was therefore honoured on no surface: the
    /// developer switched `ShadowEnabled` on and nothing anywhere changed.
    #[test]
    fn the_rail_draws_its_own_drop_shadow() {
        let ctx = ctx();
        let items = sample();
        let none: Vec<String> = Vec::new();

        let mut ctrl = Control::new("SIDE-1", crate::ControlType::SideMenu, 0, 0);
        assert!(
            state_for_control(&ctx, &ctrl, &items, 255, &none).shadow.is_none(),
            "no shadow unless the developer asked for one"
        );

        ctrl.set_prop("ShadowEnabled", true);
        ctrl.set_prop("ShadowDirection", "East");
        ctrl.set_prop("ShadowDistance", 10);
        let st = state_for_control(&ctx, &ctrl, &items, 255, &none);
        let sh = st.shadow.expect("ShadowEnabled resolves a shadow");
        assert!(!sh.is_overlay(), "a positive blur casts UNDER the rail");

        // Fading the control fades its shadow with it, once, here — the
        // painter never has to know about alpha.
        let faded = state_for_control(&ctx, &ctrl, &items, 128, &none)
            .shadow
            .expect("still a shadow");
        assert!(
            format!("{faded:?}") != format!("{sh:?}"),
            "a half-faded rail must not cast a full-strength shadow"
        );

        // A NEGATIVE blur strength is the sunken variant, drawn over the face.
        ctrl.set_prop("ShadowBlurStrength", -6);
        let sunken = state_for_control(&ctx, &ctrl, &items, 255, &none)
            .shadow
            .expect("still a shadow");
        assert!(sunken.is_overlay(), "a negative blur reads as sunken");

        // And it reaches the painter: with the shadow thrown 10pt East there
        // are shapes to the RIGHT of the rail, where nothing was drawn before.
        ctrl.set_prop("ShadowBlurStrength", 8);
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(OPEN_WIDTH, 400.0));
        let outside = Rect::from_min_max(
            Pos2::new(rect.max.x + 1.0, rect.min.y),
            Pos2::new(rect.max.x + 20.0, rect.max.y),
        );
        let painted_beyond = |ctrl: &Control| -> bool {
            let mut st = state_for_control(&ctx, ctrl, &items, 255, &none);
            st.backdrop = Color32::from_rgb(20, 22, 45);
            let rows = layout(rect, &st);
            let mut full = ctx.run_ui(egui::RawInput::default(), |ui| {
                paint(ui.painter(), rect, &rows, &st);
            });
            full.textures_delta.clear();
            full.shapes
                .iter()
                .any(|s| outside.intersects(s.shape.visual_bounding_rect()))
        };
        assert!(
            painted_beyond(&ctrl),
            "the shadow must actually be painted beside the rail"
        );
        let mut off = ctrl.clone();
        off.set_prop("ShadowEnabled", false);
        assert!(
            !painted_beyond(&off),
            "…and nothing is painted there with the shadow switched off"
        );

        // Operator report: "ShadowEnabled does nothing." With ONLY that property
        // set — no direction, no distance — the generic default is South, and a
        // full-height rail throws that below the form where nothing can see it.
        // The rail defaults to East instead, along the edge it actually has.
        {
            let mut plain = Control::new("SIDE-2", crate::ControlType::SideMenu, 0, 0);
            plain.set_prop("ShadowEnabled", true);
            assert!(
                painted_beyond(&plain),
                "ShadowEnabled alone must produce a visible shadow beside the rail"
            );
            let mut plain_off = plain.clone();
            plain_off.set_prop("ShadowEnabled", false);
            assert!(
                !painted_beyond(&plain_off),
                "…and switching it off must remove it"
            );
            // A direction the developer chooses still wins over the rail's
            // default: the resolved shadow must differ from the East one.
            let mut southward = plain.clone();
            southward.set_prop("ShadowDirection", "South");
            let defaulted = state_for_control(&ctx, &plain, &items, 255, &none)
                .shadow
                .expect("rail default");
            let chosen = state_for_control(&ctx, &southward, &items, 255, &none)
                .shadow
                .expect("explicit South");
            assert_ne!(
                format!("{defaulted:?}"),
                format!("{chosen:?}"),
                "an explicit South must be honoured, not replaced by the rail's East"
            );
            // (South's own visibility is not asserted here: a blurred shadow
            // spreads on every axis, so it clips the right-hand probe too. The
            // point is only that the developer's choice reaches the painter.)
        }

        // 050 — and the SAME under a self-contained theme. Operator report:
        // "the sidebar's shadow is fixed in Elegance". `ShadowEnabled` is the
        // developer's under every theme; a theme decides how a control LOOKS,
        // never whether a property of theirs still works.
        crate::paint::set_surface_theme(&ctx, crate::surface_theme::elegance());
        assert!(
            painted_beyond(&ctrl),
            "Elegance: the shadow must be drawn when the developer enables it"
        );
        assert!(
            !painted_beyond(&off),
            "Elegance: switching ShadowEnabled off must remove it — it is not \
             fixed by the theme"
        );
        crate::paint::set_surface_theme(&ctx, crate::surface_theme::liquid_glass());

        eprintln!(
            "049 sidebar shadow — ShadowEnabled off ⇒ none; on ⇒ cast East 10pt \
             under the rail, painted beside it; blur -6 ⇒ sunken (over the \
             face); alpha 128 ⇒ faded once, in the state"
        );
    }

    /// The rail paints headlessly in both states without panicking.
    #[test]
    fn paints_headlessly_in_both_states() {
        let items = sample();
        let none: Vec<String> = Vec::new();
        let ctx = ctx();
        for (collapsed, w) in [(false, OPEN_WIDTH), (true, COLLAPSED_WIDTH)] {
            let st = state(&items, collapsed, &none);
            let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(w, 700.0));
            let rows = layout(rect, &st);
            let mut full = ctx.run_ui(egui::RawInput::default(), |ui| {
                paint(ui.painter(), rect, &rows, &st);
            });
            full.textures_delta.clear();
        }
    }
}
