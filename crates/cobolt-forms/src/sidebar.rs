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

const ROW_H: f32 = 42.0;
const ROW_GAP: f32 = 4.0;
const SECTION_H: f32 = 30.0;
const HEADER_H: f32 = 64.0;
const PROFILE_H: f32 = 64.0;
const PAD_X: f32 = 12.0;
const ICON: f32 = 22.0;
const RADIUS: f32 = 10.0;

/// What a laid-out row IS — the caller needs this to know what a click means.
#[derive(Clone, Debug, PartialEq)]
pub enum RowKind {
    /// The logo/title block. Clicking it is a no-op today.
    Header,
    /// A section title (`HOME`) expanded, an ellipsis divider collapsed.
    Section(String),
    /// A menu item. `path` indexes into the definition (`[2]`, `[2, 0]`).
    Item { id: String, path: Vec<usize>, depth: usize },
    /// The bottom profile card.
    Profile,
}

/// One laid-out row and where it landed.
#[derive(Clone, Debug)]
pub struct SidebarRow {
    pub kind: RowKind,
    pub rect: Rect,
}

/// The chrome around the menu itself, read from the SideMenu's properties.
#[derive(Clone, Debug, Default)]
pub struct SidebarChrome {
    pub app_title: String,
    pub show_profile: bool,
    pub profile_name: String,
    pub profile_role: String,
}

impl SidebarChrome {
    /// Read the chrome properties off a SideMenu control.
    pub fn from_control(ctrl: &Control) -> Self {
        let s = |k: &str| {
            ctrl.get_prop(k)
                .map(|v| v.as_str().to_owned())
                .unwrap_or_default()
        };
        Self {
            app_title: s("AppTitle"),
            show_profile: ctrl
                .get_prop("ShowProfile")
                .map(|v| v.as_bool())
                .unwrap_or(false),
            profile_name: s("ProfileName"),
            profile_role: s("ProfileRole"),
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
    pub font: FontId,
}

/// Lay the rail out without painting: the row rectangles, in draw order.
/// Hit-testing and painting both walk this, so a click can never land on a
/// different row than the one drawn under the pointer.
pub fn layout(rect: Rect, state: &SidebarState<'_>) -> Vec<SidebarRow> {
    let mut rows = Vec::new();
    let mut y = rect.min.y;

    // Header — logo + (expanded only) the application title.
    rows.push(SidebarRow {
        kind: RowKind::Header,
        rect: Rect::from_min_size(Pos2::new(rect.min.x, y), Vec2::new(rect.width(), HEADER_H)),
    });
    y += HEADER_H;

    // The scrolling body. The profile card is anchored to the bottom, so the
    // body stops short of it.
    let body_bottom = if state.chrome.show_profile {
        rect.max.y - PROFILE_H
    } else {
        rect.max.y
    };

    fn walk(
        items: &[MenuItem],
        prefix: &mut Vec<usize>,
        depth: usize,
        y: &mut f32,
        rect: Rect,
        bottom: f32,
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
                if *y + h <= bottom {
                    rows.push(SidebarRow {
                        kind: RowKind::Section(
                            item.section_title().unwrap_or_default().to_owned(),
                        ),
                        rect: Rect::from_min_size(
                            Pos2::new(rect.min.x, *y),
                            Vec2::new(rect.width(), h),
                        ),
                    });
                }
                *y += h;
                prefix.pop();
                continue;
            }
            if *y + ROW_H <= bottom {
                rows.push(SidebarRow {
                    kind: RowKind::Item {
                        id: item.id.clone(),
                        path: prefix.clone(),
                        depth,
                    },
                    rect: Rect::from_min_size(
                        Pos2::new(rect.min.x, *y),
                        Vec2::new(rect.width(), ROW_H),
                    ),
                });
            }
            *y += ROW_H + ROW_GAP;
            // Children show only when expanded, and never on the rail.
            if !state.collapsed
                && item.has_children()
                && state.expanded.iter().any(|e| e == &item.id)
            {
                walk(&item.items, prefix, depth + 1, y, rect, bottom, state, rows);
            }
            prefix.pop();
        }
    }

    let mut prefix = Vec::new();
    walk(
        state.items,
        &mut prefix,
        0,
        &mut y,
        rect,
        body_bottom,
        state,
        &mut rows,
    );

    if state.chrome.show_profile {
        rows.push(SidebarRow {
            kind: RowKind::Profile,
            rect: Rect::from_min_size(
                Pos2::new(rect.min.x, rect.max.y - PROFILE_H),
                Vec2::new(rect.width(), PROFILE_H),
            ),
        });
    }
    rows
}

/// Which laid-out row contains `pos`, if any.
pub fn row_at(rows: &[SidebarRow], pos: Pos2) -> Option<usize> {
    rows.iter().position(|r| r.rect.contains(pos))
}

/// Paint the rail. `rows` must come from [`layout`] for the same rect/state.
pub fn paint(painter: &egui::Painter, rows: &[SidebarRow], state: &SidebarState<'_>) {
    let pal = state.palette;
    for (ix, row) in rows.iter().enumerate() {
        let hovered = state.hovered == Some(ix);
        match &row.kind {
            RowKind::Header => paint_header(painter, row.rect, state),
            RowKind::Section(title) => paint_section(painter, row.rect, title, state),
            RowKind::Item { id, path, depth } => {
                let item = item_at(state.items, path);
                if let Some(item) = item {
                    let active = state.selected == Some(id.as_str());
                    paint_item(painter, row.rect, item, *depth, active, hovered, state);
                }
            }
            RowKind::Profile => paint_profile(painter, row.rect, state),
        }
    }
    let _ = pal;
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

fn paint_header(painter: &egui::Painter, rect: Rect, state: &SidebarState<'_>) {
    let pal = state.palette;
    // The logo: a drawn mark, so a rail always has an identity even before the
    // developer supplies artwork.
    let s = 26.0_f32;
    let cx = if state.collapsed {
        rect.center().x
    } else {
        rect.min.x + PAD_X + s * 0.5
    };
    let cy = rect.center().y;
    let mark = |dx: f32, w: f32| {
        painter.add(egui::Shape::convex_polygon(
            vec![
                Pos2::new(cx + dx, cy - s * 0.42),
                Pos2::new(cx + dx + w, cy + s * 0.42),
                Pos2::new(cx + dx - w, cy + s * 0.42),
            ],
            pal.accent,
            Stroke::NONE,
        ));
    };
    mark(-s * 0.22, s * 0.26);
    mark(s * 0.22, s * 0.26);

    if !state.collapsed && !state.chrome.app_title.trim().is_empty() {
        let mut f = state.font.clone();
        f.size *= 1.45;
        painter.text(
            Pos2::new(rect.min.x + PAD_X + s + 10.0, cy),
            egui::Align2::LEFT_CENTER,
            state.chrome.app_title.trim(),
            f,
            pal.accent,
        );
    }
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
    let icon_c = if state.collapsed {
        rect.center()
    } else {
        Pos2::new(rect.min.x + PAD_X + indent + ICON * 0.5, rect.center().y)
    };

    match &item.icon {
        Some(name) => {
            let mut style = state.icon_style;
            style.color = content;
            crate::icons::draw_menu_icon_styled(
                painter,
                Rect::from_center_size(icon_c, Vec2::splat(ICON)),
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
    let label_x = rect.min.x + PAD_X + indent + ICON + 10.0;
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

fn paint_profile(painter: &egui::Painter, rect: Rect, state: &SidebarState<'_>) {
    let pal = state.palette;
    let card = if state.collapsed {
        Rect::from_center_size(rect.center(), Vec2::splat(ROW_H - 6.0))
    } else {
        Rect::from_min_max(
            Pos2::new(rect.min.x + PAD_X * 0.5, rect.min.y + 4.0),
            Pos2::new(rect.max.x - PAD_X * 0.5, rect.max.y - 8.0),
        )
    };
    painter.rect_filled(card, RADIUS, pal.hover);

    let av_r = 15.0;
    let av_c = if state.collapsed {
        card.center()
    } else {
        Pos2::new(card.min.x + 10.0 + av_r, card.center().y)
    };
    painter.circle_filled(av_c, av_r, pal.accent);
    // A drawn head-and-shoulders stands in until the developer sets an avatar.
    painter.circle_filled(
        Pos2::new(av_c.x, av_c.y - av_r * 0.22),
        av_r * 0.34,
        pal.on_accent,
    );
    let body = Rect::from_center_size(
        Pos2::new(av_c.x, av_c.y + av_r * 0.62),
        Vec2::new(av_r * 1.05, av_r * 0.62),
    );
    painter.rect_filled(body, av_r * 0.31, pal.on_accent);

    if state.collapsed {
        return;
    }
    let x = av_c.x + av_r + 10.0;
    let mut nf = state.font.clone();
    nf.size *= 1.05;
    painter.text(
        Pos2::new(x, card.center().y - 8.0),
        egui::Align2::LEFT_CENTER,
        state.chrome.profile_name.trim(),
        nf,
        pal.fg,
    );
    let mut rf = state.font.clone();
    rf.size *= 0.82;
    painter.text(
        Pos2::new(x, card.center().y + 9.0),
        egui::Align2::LEFT_CENTER,
        state.chrome.profile_role.trim(),
        rf,
        pal.dim,
    );
    // The circular action affordance on the right of the card.
    let ac = Pos2::new(card.max.x - 20.0, card.center().y);
    painter.circle_stroke(ac, 9.0, Stroke::new(1.6, pal.accent));
    painter.circle_filled(ac, 3.4, pal.accent);
}

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
                show_profile: true,
                profile_name: "Mathew".into(),
                profile_role: "Designer".into(),
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
        assert!(matches!(rows.last().unwrap().kind, RowKind::Profile));
        for r in &rows {
            assert!(
                rect.contains_rect(r.rect),
                "{:?} escaped the rail: {:?}",
                r.kind,
                r.rect
            );
        }
        eprintln!(
            "049 sidebar layout — {} rows inside a {}x{} rail: header + 2 sections \
             + 4 items + profile, none escaping",
            rows.len(),
            OPEN_WIDTH,
            700.0
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
                paint(ui.painter(), &rows, &st);
            });
            full.textures_delta.clear();
        }
    }
}
