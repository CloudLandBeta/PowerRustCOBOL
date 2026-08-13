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
pub const COLLAPSED_WIDTH: f32 = 72.0;

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
const ICON: f32 = 22.0;
const RADIUS: f32 = 10.0;

/// The header logo's box, in points. A `HeaderImage` is STRETCHED to fill
/// exactly this, and a header with none outlines it — so the developer designs
/// their logo against a size they can see, instead of discovering it when the
/// application runs.
pub const HEADER_IMAGE_W: f32 = 200.0;
pub const HEADER_IMAGE_H: f32 = 60.0;

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
    /// Menu-item icon size in points (the control's `IconSize`). Icons are
    /// vectors, so this scales cleanly at any value.
    pub icon_size: f32,
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
        icon_size: ctrl
            .get_prop("IconSize")
            .map(|v| v.as_i64() as f32)
            .filter(|s| *s >= 4.0)
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
            ctrl,
            crate::paint::glass_config_applies(ctx)
                && crate::paint::active_glass_style(ctx).is_neumorphic(),
        )
        .map(|s| s.faded(alpha as f32 / 255.0)),
        font,
    }
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
    let mut prefix = Vec::new();
    walk_rows(state.items, &mut prefix, 0, &mut y, rect, state, &mut rows);

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

/// Lay out the menu rows from `y` downwards, recursing into expanded parents.
/// Rows are produced unconditionally — confining them to the pane is the
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
    let x = if collapsed {
        header.center().x - size.x * 0.5
    } else {
        header.min.x + PAD_X
    };
    Rect::from_min_size(Pos2::new(x, header.center().y - size.y * 0.5), size)
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
            painter.image(
                tex,
                logo,
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
    let indent = depth as f32 * 16.0;
    let icon = state.icon_size;
    let icon_c = if state.collapsed {
        rect.center()
    } else {
        Pos2::new(rect.min.x + PAD_X + indent + icon * 0.5, rect.center().y)
    };

    match &item.icon {
        Some(name) => {
            let mut style = state.icon_style;
            style.color = content;
            crate::icons::draw_menu_icon_styled(
                painter,
                Rect::from_center_size(icon_c, Vec2::splat(icon)),
                name,
                &style,
            );
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

    // Label.
    let label_x = rect.min.x + PAD_X + indent + icon + 10.0;
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

    fn sample() -> Vec<MenuItem> {
        let mut home = MenuItem::new_separator("s1");
        home.label = "Home".into();
        let mut modern = MenuItem::new_action("modern", "Modern");
        modern.icon = Some("home".into());
        modern.badge = Some("New".into());
        let mut analytical = MenuItem::new_action("analytical", "Analytical");
        analytical.icon = Some("chart-bar".into());
        let mut apps = MenuItem::new_separator("s2");
        apps.label = "Apps".into();
        let mut chat = MenuItem::new_action("chat", "Chat");
        chat.icon = Some("chat".into());
        chat.badge = Some("6".into());
        chat.badge_style = BadgeStyle::Count;
        let mut level = MenuItem::new_action("level", "Menu Level");
        level.items.push(MenuItem::new_action("sub", "Salma"));
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

    /// The collapsed rail keeps every item reachable and turns section titles
    /// into ellipsis dividers.
    #[test]
    fn collapsed_rail_keeps_items_and_uses_ellipsis_dividers() {
        let items = sample();
        let none: Vec<String> = Vec::new();
        let st = state(&items, true, &none);
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(COLLAPSED_WIDTH, 700.0));
        let rows = layout(rect, &st);
        let n_items = rows
            .iter()
            .filter(|r| matches!(r.kind, RowKind::Item { .. }))
            .count();
        assert_eq!(n_items, 4, "every top-level item survives the rail");
        assert_eq!(
            rows.iter()
                .filter(|r| matches!(&r.kind, RowKind::Section(t) if !t.is_empty()))
                .count(),
            2,
            "sections become the rail's group dividers"
        );
        for r in &rows {
            assert!(rect.contains_rect(r.rect), "{:?} escaped the rail", r.kind);
        }
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

    /// The logo box is a fixed 200×60 the image is stretched into, so what the
    /// developer designs against is what ships. It shrinks — keeping its
    /// shape — only when the rail or the header cannot hold it.
    #[test]
    fn the_header_logo_box_is_a_fixed_200x60_that_shrinks_in_shape() {
        // A rail wide and tall enough: the box is exactly the contract.
        let header = Rect::from_min_size(Pos2::ZERO, Vec2::new(240.0, 72.0));
        let r = header_image_rect(header, false);
        assert_eq!((r.width(), r.height()), (HEADER_IMAGE_W, HEADER_IMAGE_H));
        assert_eq!((r.width(), r.height()), (200.0, 60.0), "the stated size");
        assert_eq!(r.min.x, header.min.x + PAD_X, "left-aligned when open");
        assert!(header.contains_rect(r), "and inside the header pane");

        let aspect = HEADER_IMAGE_W / HEADER_IMAGE_H;
        // The collapsed rail is a third of the box's width: it shrinks, centred,
        // and keeps its shape rather than squashing the logo.
        let rail = Rect::from_min_size(Pos2::ZERO, Vec2::new(COLLAPSED_WIDTH, 64.0));
        let r = header_image_rect(rail, true);
        assert!(r.width() < HEADER_IMAGE_W, "shrunk to the rail");
        assert!((r.width() / r.height() - aspect).abs() < 0.01, "shape kept");
        assert!(
            (r.center().x - rail.center().x).abs() < 0.5,
            "centred on the collapsed rail"
        );
        assert!(rail.contains_rect(r));

        // A header SHORTER than the box shrinks it too — height binds first.
        let short = Rect::from_min_size(Pos2::ZERO, Vec2::new(240.0, 40.0));
        let r = header_image_rect(short, false);
        assert!(r.height() <= short.height() - 8.0, "fits the short header");
        assert!((r.width() / r.height() - aspect).abs() < 0.01, "shape kept");
        assert!(short.contains_rect(r));

        eprintln!(
            "049 sidebar header logo — 240x72 rail → {:.0}x{:.0} (the 200x60 \
             contract); {:.0}px collapsed rail → {:.0}x{:.0}; 40px header → \
             {:.0}x{:.0}; aspect held at {aspect:.2} throughout",
            HEADER_IMAGE_W,
            HEADER_IMAGE_H,
            COLLAPSED_WIDTH,
            header_image_rect(rail, true).width(),
            header_image_rect(rail, true).height(),
            header_image_rect(short, false).width(),
            header_image_rect(short, false).height(),
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
