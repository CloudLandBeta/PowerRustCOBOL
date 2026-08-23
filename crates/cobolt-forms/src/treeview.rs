// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The TreeView's layout and paint — **one implementation**, for the designer
//! canvas and the running form alike.
//!
//! Before this the two disagreed completely: the canvas drew the placeholder
//! caption `🌲 [TreeView]` and no nodes at all, while the running form drew a
//! flat bulleted list in a fixed 12pt font. Everything the inspector offered —
//! `ShowLines`, `ShowRootLines`, `LineColor`, `CheckBoxes`, `Sorted`,
//! `HotTracking` — reached neither: eight rows, and not one of them changed a
//! pixel (operator, 2026-08-22: "treeview not working / content not rendered").
//!
//! `Items` is the tree, one node per line, **two spaces per level**:
//!
//! ```text
//! Node 1
//!   Child 1
//!     Grandchild
//! Node 2
//! ```
//!
//! A row's index is its line's position in `Items` as WRITTEN — never its
//! position after sorting — so an event names the node the developer wrote and
//! `Sorted` cannot renumber anybody's handler.

use crate::model::Control;
use egui::{Color32, Pos2, Rect, Stroke, Vec2};

/// Row pitch, first-row offset, indent, check box and icon — the DEFAULTS
/// behind `RowHeight`, `IndentWidth`, `CheckBoxSize` and `IconSize`. Each is
/// what the running form already used, so a tree that sets none of them does
/// not move.
pub const ROW_H: f32 = 18.0;
const FIRST_ROW_Y: f32 = 12.0;
pub const INDENT: f32 = 16.0;
const CHECK: f32 = 12.0;
const ICON: f32 = 14.0;

/// Read a numeric property, clamped to something a control can actually draw.
///
/// Through `as_i64`, not `as_str`: a `PropValue::Int` answers the empty string
/// to `as_str`, so reading these as text found nothing and every metric sat on
/// its default however the inspector was set. A value typed as text still
/// parses — `as_i64` reads a `String` too — so both spellings work.
fn num(ctrl: &Control, key: &str, default: f32, lo: f32, hi: f32) -> f32 {
    ctrl.get_prop(key)
        .map(|v| match v {
            crate::PropValue::String(s) => s.trim().parse::<f32>().unwrap_or(default),
            other => other.as_i64() as f32,
        })
        .filter(|n| n.is_finite() && *n > 0.0)
        .unwrap_or(default)
        .clamp(lo, hi)
}

/// The metrics a tree is laid out and painted with — every one a property, so
/// nothing here is a number the developer cannot reach.
#[derive(Clone, Copy, Debug)]
pub struct TreeMetrics {
    pub row_h: f32,
    pub indent: f32,
    pub check: f32,
    pub icon: f32,
}

impl TreeMetrics {
    pub fn of(ctrl: &Control) -> Self {
        let icon = num(ctrl, "IconSize", ICON, 6.0, 64.0);
        let check = num(ctrl, "CheckBoxSize", CHECK, 6.0, 64.0);
        // `NodeSpacing` is the GAP between one node and the next, on top of
        // whatever the row itself needs — the property a developer reaches for
        // when the rows read as a wall of text.
        let spacing = ctrl
            .get_prop("NodeSpacing")
            .map(|v| match v {
                crate::PropValue::String(s) => s.trim().parse::<f32>().unwrap_or(0.0),
                other => other.as_i64() as f32,
            })
            .filter(|n| n.is_finite() && *n >= 0.0)
            .unwrap_or(0.0)
            .clamp(0.0, 100.0);
        // A row is never shorter than what it must hold. Growing the icon used
        // to leave the pitch alone, so a big icon painted over the node above
        // and below it (operator, 2026-08-22) — the row now grows with whatever
        // is tallest on it, and `RowHeight` sets the FLOOR rather than the
        // ceiling.
        let content = icon.max(check) + 4.0;
        Self {
            row_h: num(ctrl, "RowHeight", ROW_H, 8.0, 200.0).max(content) + spacing,
            indent: num(ctrl, "IndentWidth", INDENT, 0.0, 200.0),
            check,
            icon,
        }
    }
}

/// One laid-out node.
#[derive(Clone, Debug, PartialEq)]
pub struct TreeRow {
    /// Position of this node's line in `Items` as written — the id an event
    /// carries, unchanged by `Sorted` and by anything being collapsed.
    pub index: usize,
    pub depth: usize,
    pub text: String,
    /// The icon this node draws, from the platform's own catalogue.
    pub icon: Option<String>,
    /// This node's OWN label colour, when it named one. `None` follows the
    /// tree's ink, which is what every node did before a node could speak for
    /// itself.
    pub color: Option<String>,
    /// This node's OWN row colour, when it named one — painted under the
    /// selection and hot-track bands, so a coloured row still shows which row
    /// is selected.
    pub background: Option<String>,
    /// Whether anything is written under it — what earns a disclosure arrow.
    pub has_children: bool,
    /// Whether its own children are hidden right now.
    pub collapsed: bool,
    /// The full-width band: what a click, a selection and a hot-track use.
    pub rect: Rect,
    /// The disclosure arrow, on a node that has children.
    pub expander: Option<Rect>,
    /// The node's icon.
    pub icon_rect: Option<Rect>,
    /// Where the label starts.
    pub label_x: f32,
    /// The check box, when `CheckBoxes` is on.
    pub check: Option<Rect>,
}

/// The tree ITSELF lives in [`crate::treenodes`], outside the `render` feature,
/// so the interpreter can answer a handler's `NodeParent` without egui — and
/// answer it from the same parse the canvas draws from. Re-exported here
/// because this is where a reader looks for it.
pub use crate::treenodes::{index_of, node_at, nodes, NodeInfo};
use crate::treenodes::{parse, sort_siblings};

fn flag(ctrl: &Control, key: &str, default: bool) -> bool {
    ctrl.get_prop(key).map(|v| v.as_bool()).unwrap_or(default)
}

/// Where a row's own drawing starts — its arrow, its tick box or its icon,
/// whichever is leftmost, falling back to the label.
///
/// What the connector lines must stop at. They used to stop at the LABEL, which
/// put the elbow straight through the icon between them.
fn row_left_edge(row: &TreeRow) -> f32 {
    [
        row.expander.map(|r| r.min.x),
        row.check.map(|r| r.min.x),
        row.icon_rect.map(|r| r.min.x),
    ]
    .into_iter()
    .flatten()
    .fold(row.label_x, f32::min)
}

/// A colour the developer named, used EXACTLY as given — alpha included, since
/// a selection band is mostly alpha. `None` when they named none, which is what
/// lets every colour property here mean "leave it to the tree" while empty.
fn named_color(ctrl: &Control, key: &str) -> Option<Color32> {
    ctrl.get_prop(key)
        .map(|v| v.as_str().trim().to_owned())
        .filter(|s| !s.is_empty())
        .map(|s| crate::paint::parse_color(&s))
        .filter(|c| c.a() > 0)
}

/// The tick box's rim — `(style, width, colour)` — read from the same three
/// keys a CheckBox uses, with the TREE's defaults rather than the CheckBox's.
///
/// The box has always been drawn with a 1px rim in the node ink, so that is
/// what an untouched tree keeps; an empty colour follows the ink, which is the
/// rule `IconColor` already set here. Only the DEFAULTS differ from a check
/// box's — the property names, and what each one means, are identical.
fn check_box_border(ctrl: &Control, ink: Color32) -> (String, f32, Color32) {
    let style = ctrl
        .get_prop("CheckBoxBorderStyle")
        .map(|v| v.as_str().trim().to_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Single".into());
    let width = num(ctrl, "CheckBoxBorderWidth", 1.0, 0.0, 20.0);
    let colour = named_color(ctrl, "CheckBoxBorderColor").unwrap_or(ink);
    (style, width, colour)
}

/// One node the tree actually SHOWS, with the two facts the walk worked out.
struct VisibleNode {
    node: crate::treenodes::ParsedNode,
    has_children: bool,
    collapsed: bool,
}

/// The nodes a tree shows: sorted if asked, with everything inside a folded
/// node left out.
///
/// Shared by the layout and by [`content_height`], so how far a tree can scroll
/// is measured against the rows it actually draws. Counting the raw lines
/// instead would let a tree folded down to three rows still scroll as if it
/// held three hundred.
fn visible_nodes(ctrl: &Control) -> Vec<VisibleNode> {
    let items = ctrl
        .get_prop("Items")
        .map(|v| v.as_str().to_owned())
        .unwrap_or_default();
    let mut nodes = parse(&items);
    if flag(ctrl, "Sorted", false) {
        nodes = sort_siblings(nodes);
    }
    // `CollapsedNodes` rather than an expanded list, so EMPTY means the whole
    // tree is open — which is what a tree has always shown, and what a
    // developer who has never heard of the property still gets.
    let collapsed: Vec<String> = ctrl
        .get_prop("CollapsedNodes")
        .map(|v| {
            v.as_str()
                .lines()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    let mut out = Vec::new();
    // Everything deeper than this is inside something folded shut.
    let mut hide_below: Option<usize> = None;
    for (n, node) in nodes.iter().enumerate() {
        match hide_below {
            Some(d) if node.depth > d => continue,
            Some(_) => hide_below = None,
            None => {}
        }
        // A node HAS children when the next line is deeper than it — the only
        // thing that earns a disclosure arrow.
        let has_children = nodes
            .get(n + 1)
            .map(|next| next.depth > node.depth)
            .unwrap_or(false);
        let is_collapsed = has_children && collapsed.iter().any(|c| *c == node.text);
        if is_collapsed {
            hide_below = Some(node.depth);
        }
        out.push(VisibleNode {
            node: node.clone(),
            has_children,
            collapsed: is_collapsed,
        });
    }
    out
}

/// How tall the tree is in total — every row it shows, scroll or no scroll.
pub fn content_height(ctrl: &Control) -> f32 {
    let n = visible_nodes(ctrl).len();
    if n == 0 {
        0.0
    } else {
        // The half row above the first centre, every row's pitch, and the same
        // half again under the last — so the last node is not flush against the
        // bottom edge when scrolled all the way down.
        FIRST_ROW_Y + n as f32 * TreeMetrics::of(ctrl).row_h
    }
}

/// How far the tree CAN scroll: zero when everything already fits.
pub fn max_scroll(ctrl: &Control, rect: Rect) -> f32 {
    (content_height(ctrl) - rect.height()).max(0.0)
}

/// A scroll offset held inside what there is to scroll — the guard that stops
/// a wheel or a drag running past either end.
pub fn clamp_scroll(ctrl: &Control, rect: Rect, scroll: f32) -> f32 {
    scroll.clamp(0.0, max_scroll(ctrl, rect))
}

/// The label of every row the tree SHOWS, top to bottom — including the rows
/// currently scrolled out of sight.
///
/// What an arrow key steps through. Stepping through the laid-out rows instead
/// would stop the selection dead at the viewport edge, which is precisely the
/// moment it needs to keep going and drag the view with it.
pub fn visible_labels(ctrl: &Control) -> Vec<String> {
    visible_nodes(ctrl)
        .into_iter()
        .map(|v| v.node.text)
        .collect()
}

/// The scroll that brings row `n` (counted among the rows the tree SHOWS) fully
/// inside `rect`, leaving it alone when it is already there.
///
/// This is what makes a keyboard selection reachable: the arrow moves the
/// selection, and the view follows only as far as it must.
pub fn scroll_to_row(ctrl: &Control, rect: Rect, n: usize, scroll: f32) -> f32 {
    let m = TreeMetrics::of(ctrl);
    let centre = FIRST_ROW_Y + n as f32 * m.row_h;
    let (top, bottom) = (centre - m.row_h * 0.5, centre + m.row_h * 0.5);
    // Below the fold: scroll just enough that its bottom sits on the edge.
    // Above it: just enough that its top does. `clamp` needs its low bound
    // under its high one, which it is whenever a row is shorter than the
    // control — and when it is not, showing the row's TOP is the better answer.
    let low = (bottom - rect.height()).min(top);
    clamp_scroll(ctrl, rect, scroll.clamp(low, top))
}

/// Lay the tree out inside `rect`, unscrolled — what a design surface draws.
pub fn layout(ctrl: &Control, rect: Rect) -> Vec<TreeRow> {
    layout_at(ctrl, rect, 0.0)
}

/// Lay the tree out inside `rect`, `scroll` points from the top.
///
/// Rows that fall entirely outside `rect` are dropped — above it as well as
/// below, now that there is an above. A row that straddles an edge is KEPT and
/// clipped by the painter: dropping it would make the tree jump a whole row at
/// a time instead of sliding, and a half-row at the edge is how an operator
/// knows there is more to come.
pub fn layout_at(ctrl: &Control, rect: Rect, scroll: f32) -> Vec<TreeRow> {
    let visible = visible_nodes(ctrl);
    let m = TreeMetrics::of(ctrl);
    let checks = flag(ctrl, "CheckBoxes", false);
    let icons = flag(ctrl, "ShowIcons", true);
    let mut y = rect.min.y + FIRST_ROW_Y - clamp_scroll(ctrl, rect, scroll);
    let mut rows = Vec::new();
    for VisibleNode {
        node,
        has_children,
        collapsed: is_collapsed,
    } in &visible
    {
        let (index, depth, text) = (node.index, node.depth, node.text.clone());
        let (has_children, is_collapsed) = (*has_children, *is_collapsed);
        let icon = &node.icon;
        // Above the top: step past it without building it, so scrolling costs
        // the same whether the operator is at the first row or the ten
        // thousandth.
        if y + m.row_h * 0.5 < rect.min.y {
            y += m.row_h;
            continue;
        }
        if y - m.row_h * 0.5 > rect.max.y {
            break;
        }
        let band = Rect::from_min_max(
            Pos2::new(rect.min.x + 2.0, y - m.row_h * 0.5),
            Pos2::new(rect.max.x - 2.0, y + m.row_h * 0.5),
        );
        // The row reads left to right: arrow, tick box, icon, label. Each part
        // takes its room only when it is there.
        let mut x = rect.min.x + 8.0 + depth as f32 * m.indent;
        // The arrow's slot is reserved on EVERY row, drawn only where there is
        // something to fold. Reserving it only for parents let a leaf's label
        // slide left of its own siblings' — and a deeper leaf left of its own
        // parent's, which reads as the wrong depth. Labels line up in a column
        // because the slot is always there, which is why every tree view does
        // this.
        let arrow_slot = Rect::from_center_size(
            Pos2::new(x + m.icon * 0.5, y),
            Vec2::splat(m.icon),
        );
        x = arrow_slot.max.x + 4.0;
        let expander = has_children.then_some(arrow_slot);
        let check = checks.then(|| {
            let r = Rect::from_center_size(
                Pos2::new(x + m.check * 0.5, y),
                Vec2::splat(m.check),
            );
            x = r.max.x + 6.0;
            r
        });
        // An explicit icon leads; otherwise a node that holds others is a
        // folder — open or shut to match its own arrow — and a leaf is a
        // document. Both are the platform's own icons, not new artwork.
        let icon_name = icons.then(|| match icon {
            Some(name) => name.clone(),
            None if has_children && is_collapsed => {
                default_icon(ctrl, "ParentIcon", "folder")
            }
            None if has_children => default_icon(ctrl, "ParentIconOpen", "folder-open"),
            None => default_icon(ctrl, "LeafIcon", "doc-text"),
        });
        let icon_rect = icon_name.as_ref().map(|_| {
            let r = Rect::from_center_size(
                Pos2::new(x + m.icon * 0.5, y),
                Vec2::splat(m.icon),
            );
            x = r.max.x + 5.0;
            r
        });
        rows.push(TreeRow {
            index,
            depth,
            text,
            icon: icon_name,
            color: node.color.clone(),
            background: node.background.clone(),
            has_children,
            collapsed: is_collapsed,
            rect: band,
            expander,
            icon_rect,
            label_x: x,
            check,
        });
        y += m.row_h;
    }
    rows
}

/// A defaulted icon-name property — empty falls back to the platform's own
/// choice rather than to no icon, so a tree looks like a tree out of the box.
fn default_icon(ctrl: &Control, key: &str, built_in: &str) -> String {
    ctrl.get_prop(key)
        .map(|v| v.as_str().trim().to_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| built_in.to_owned())
}

/// What the tree looks like right now, beyond its designed properties.
#[derive(Default, Clone, Copy)]
pub struct TreeState<'a> {
    /// `SelectedNode` — the node whose band is highlighted.
    pub selected: &'a str,
    /// `CheckedNodes`, one per line — which boxes are ticked.
    pub checked: &'a [String],
    /// Row under the pointer, for `HotTracking`. The canvas passes `None`: a
    /// design surface has no pointer of its own.
    pub hovered: Option<usize>,
    /// Fade from the render walk (a faded container dims its subtree).
    pub alpha: f32,
}

/// Paint the rows into `rect`. The FACE is the caller's business — both call
/// sites draw the control's designed face first, so this adds only the tree.
pub fn paint(
    painter: &egui::Painter,
    ctrl: &Control,
    rect: Rect,
    rows: &[TreeRow],
    state: TreeState<'_>,
) {
    // Everything below is drawn INSIDE the control, whatever the scroll. A row
    // straddling an edge is deliberately kept — that is what makes the tree
    // slide instead of jumping a whole row at a time — and this is what keeps
    // its other half off the form around it.
    let painter = &painter.with_clip_rect(rect);
    let a = state.alpha.clamp(0.0, 1.0);
    let fade = |c: Color32| {
        Color32::from_rgba_premultiplied(c.r(), c.g(), c.b(), (c.a() as f32 * a) as u8)
    };
    let ink = crate::paint::treeview_ink(painter.ctx(), ctrl);
    let m = TreeMetrics::of(ctrl);
    let line_color = ctrl
        .get_prop("LineColor")
        .map(|v| crate::paint::parse_color(v.as_str()))
        .filter(|c| c.a() > 0)
        .unwrap_or(Color32::from_gray(170));
    // The icon's own colour, and the arrow's. Empty follows the node ink, so a
    // tree that is legible is a tree whose icons are legible.
    let icon_color = ctrl
        .get_prop("IconColor")
        .map(|v| v.as_str().trim().to_owned())
        .filter(|s| !s.is_empty())
        .map(|s| crate::paint::parse_color(&s))
        .filter(|c| c.a() > 0)
        .unwrap_or(ink);
    let show_lines = flag(ctrl, "ShowLines", true);
    let show_root = flag(ctrl, "ShowRootLines", true);

    // ── The tree's own lines ────────────────────────────────────────────────
    //
    // Drawn from the ROWS rather than from the text, so what is joined is
    // exactly what is drawn. A node's elbow reaches left to its parent's
    // indent column; the verticals run from a parent down to its last child.
    if show_lines {
        let stroke = Stroke::new(1.0, fade(line_color));
        for (i, row) in rows.iter().enumerate() {
            if row.depth == 0 {
                continue; // a root has no parent to join
            }
            let x = rect.min.x + 8.0 + (row.depth - 1) as f32 * m.indent + m.indent * 0.5;
            let y = row.rect.center().y;
            // Stop at whatever the row starts WITH — its arrow, its tick box or
            // its icon, whichever comes first. Running to `label_x` drew the
            // elbow straight THROUGH the icon (operator, 2026-08-22): the icon
            // sits between the connector and the label, and a line crossing it
            // reads as a scribble over the picture.
            let left = row_left_edge(row) - 4.0;
            painter.line_segment([Pos2::new(x, y), Pos2::new(left, y)], stroke);
            // The vertical to the previous sibling (or the parent) at this
            // depth — walk back until something at this depth or shallower.
            let mut top = row.rect.min.y;
            for prev in rows[..i].iter().rev() {
                if prev.depth < row.depth {
                    top = prev.rect.center().y;
                    break;
                }
                if prev.depth == row.depth {
                    top = prev.rect.center().y;
                    break;
                }
            }
            painter.line_segment([Pos2::new(x, top), Pos2::new(x, y)], stroke);
        }
    }
    // The root spine: one vertical joining the top-level nodes, which is what
    // `ShowRootLines` means and why it is separate from `ShowLines`.
    if show_root {
        let roots: Vec<&TreeRow> = rows.iter().filter(|r| r.depth == 0).collect();
        if roots.len() > 1 {
            let x = rect.min.x + 8.0 - 4.0;
            let stroke = Stroke::new(1.0, fade(line_color));
            painter.line_segment([
                Pos2::new(x, roots[0].rect.center().y),
                Pos2::new(x, roots[roots.len() - 1].rect.center().y),
            ], stroke);
            for r in roots {
                painter.line_segment(
                    [
                        Pos2::new(x, r.rect.center().y),
                        Pos2::new(row_left_edge(r) - 4.0, r.rect.center().y),
                    ],
                    stroke,
                );
            }
        }
    }

    // ── Bands, boxes and labels ─────────────────────────────────────────────
    let focus = crate::paint::theme_token(painter.ctx(), crate::surface_theme::ColorToken::Focus);
    let font = crate::fonts::font_id(
        painter.ctx(),
        &ctrl
            .get_prop("FontName")
            .map(|v| v.as_str().to_owned())
            .unwrap_or_default(),
        crate::paint::ctrl_font_size(ctrl),
    );
    for row in rows {
        let selected = !state.selected.is_empty() && state.selected == row.text;
        // The node's OWN row colour, UNDER the bands rather than instead of
        // them: a row a handler painted red must still show that it is the
        // selected row, and a selection band is mostly alpha, so the two layer
        // the way they read.
        if let Some(bg) = row
            .background
            .as_deref()
            .map(crate::paint::parse_color)
            .filter(|c| c.a() > 0)
        {
            painter.rect_filled(row.rect, 3.0, fade(bg));
        }
        if selected {
            // `SelectionColor` when the developer named one — used exactly as
            // given, alpha and all. Empty keeps the theme's focus colour at the
            // weight a selection band has always had.
            let band = named_color(ctrl, "SelectionColor").unwrap_or_else(|| {
                let c = focus.unwrap_or(Color32::from_rgb(70, 110, 200));
                Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), 70)
            });
            painter.rect_filled(row.rect, 3.0, fade(band));
        } else if state.hovered == Some(row.index) {
            // HotTracking: the row under the pointer lifts, faintly — half the
            // selection's weight by default, so the two are never confused.
            let band = named_color(ctrl, "HotTrackColor")
                .unwrap_or(Color32::from_white_alpha(18));
            painter.rect_filled(row.rect, 3.0, fade(band));
        }
        // The tick box wears the CheckBox's own five properties — its fill, its
        // rim's style, width and colour, and the tick's colour and size. Before
        // this the box was three hard numbers: a black-alpha well, a 1px rim in
        // the node ink and a tick at 28 % of the box, none of them reachable.
        if let Some(box_rect) = row.check {
            let ticked = state.checked.iter().any(|c| c == &row.text);
            // Empty means "not chosen" — the same convention the CheckBox uses,
            // and why the shared reader is called rather than copied. Nothing
            // chosen keeps the recessed well the tree has always drawn.
            let fill = crate::paint::user_checkbox_color(ctrl)
                .unwrap_or(Color32::from_black_alpha(40));
            painter.rect_filled(box_rect, 2.0, fade(fill));
            // The rim through the shared border painter, so Single, Double and
            // Dashed mean here exactly what they mean on every other control —
            // and so `None` can now switch it off.
            let (style, width, colour) = check_box_border(ctrl, ink);
            crate::paint::draw_control_border(
                painter,
                box_rect,
                egui::CornerRadius::same(2),
                &style,
                width,
                fade(colour),
            );
            if ticked {
                // The CheckBox's own tick geometry, to the coefficient: two
                // segments through a box scaled by `CheckSize`. Shared shape,
                // not a lookalike — a tick in a tree and a tick in a check box
                // are the same mark.
                let mark = named_color(ctrl, "CheckColor").unwrap_or(ink);
                let d = box_rect.width();
                let pct = num(ctrl, "CheckSize", 70.0, 10.0, 100.0) / 100.0;
                let stroke = Stroke::new((d * 0.16 * pct).clamp(1.5, 6.0), fade(mark));
                let pt = |ux: f32, uy: f32| {
                    Pos2::new(
                        box_rect.center().x + (ux - 0.5) * d * pct,
                        box_rect.center().y + (uy - 0.5) * d * pct,
                    )
                };
                painter.line_segment([pt(0.18, 0.52), pt(0.42, 0.76)], stroke);
                painter.line_segment([pt(0.42, 0.76), pt(0.84, 0.22)], stroke);
            }
        }
        // The disclosure arrow: the platform's own chevron, pointing right when
        // the node is shut and down when it is open — the direction every tree
        // uses, so nobody has to be told which way is which.
        if let Some(arrow) = row.expander {
            crate::icons::draw_menu_icon(
                painter,
                arrow,
                if row.collapsed {
                    "chevron-right"
                } else {
                    "chevron-down"
                },
                fade(icon_color),
            );
        }
        // The node's icon, from the catalogue every menu and toolbar draws
        // from. An unknown name draws nothing rather than a placeholder: a
        // typo should cost its own icon, not the row.
        if let (Some(r), Some(name)) = (row.icon_rect, row.icon.as_deref()) {
            crate::icons::draw_menu_icon(painter, r, name, fade(icon_color));
        }
        // A node that named its own colour is written in it; the rest follow
        // the tree's ink, which is already picked for contrast against the face.
        let node_ink = row
            .color
            .as_deref()
            .map(crate::paint::parse_color)
            .filter(|c| c.a() > 0)
            .unwrap_or(ink);
        painter.text(
            Pos2::new(row.label_x, row.rect.center().y),
            egui::Align2::LEFT_CENTER,
            &row.text,
            font.clone(),
            fade(node_ink),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ControlType, PropValue};

    fn tree(items: &str) -> Control {
        let mut c = Control::new("TV-1", ControlType::TreeView, 0, 0);
        c.rect = crate::model::Rect::new(0, 0, 200, 160);
        c.set_prop("Items", PropValue::String(items.into()));
        c
    }

    fn rect() -> Rect {
        Rect::from_min_size(Pos2::ZERO, Vec2::new(200.0, 160.0))
    }

    /// Indentation IS the tree: two spaces (or one tab) per level, and a node's
    /// row is indented by its depth.
    #[test]
    fn two_spaces_make_a_child() {
        let rows = layout(&tree("Root\n  Child\n    Grandchild\nOther"), rect());
        assert_eq!(
            rows.iter().map(|r| r.depth).collect::<Vec<_>>(),
            vec![0, 1, 2, 0]
        );
        assert!(
            rows[1].label_x > rows[0].label_x && rows[2].label_x > rows[1].label_x,
            "each level must sit further right: {:?}",
            rows.iter().map(|r| r.label_x).collect::<Vec<_>>()
        );
    }

    /// **`Sorted` orders SIBLINGS**, and leaves every child under the parent it
    /// was written under. A flat sort would put the nodes in order and the tree
    /// in ruins.
    #[test]
    fn sorted_orders_siblings_without_stealing_children() {
        let mut c = tree("Zebra\n  zulu\n  alpha\nApple\n  pear");
        c.set_prop("Sorted", PropValue::Bool(true));
        let rows = layout(&c, rect());
        assert_eq!(
            rows.iter().map(|r| r.text.as_str()).collect::<Vec<_>>(),
            vec!["Apple", "pear", "Zebra", "alpha", "zulu"],
            "roots sorted, and each child still under its own parent"
        );
        // The index is the line as WRITTEN, so an event names the node the
        // developer wrote whatever the sort did.
        let apple = rows.iter().find(|r| r.text == "Apple").expect("Apple");
        assert_eq!(apple.index, 3, "Apple is the fourth line of Items");
    }

    /// `CheckBoxes` gives every node a box, and the label steps aside for it.
    #[test]
    fn check_boxes_take_their_own_room() {
        let plain = layout(&tree("A\nB"), rect());
        let mut c = tree("A\nB");
        c.set_prop("CheckBoxes", PropValue::Bool(true));
        let checked = layout(&c, rect());
        assert!(plain[0].check.is_none());
        let box_rect = checked[0].check.expect("a box per node");
        assert!(
            checked[0].label_x > box_rect.max.x,
            "the label must clear its own box"
        );
        assert!(
            checked[0].label_x > plain[0].label_x,
            "…which moves it right of where it sits without one"
        );
    }

    /// **Collapsing a node hides its whole subtree**, however deep — and the
    /// node itself stays, which is the difference between folding and deleting.
    ///
    /// `CollapsedNodes` is a list of what is SHUT, so an empty one is a tree
    /// fully open: what a tree has always shown, and what a developer who never
    /// touches the property still gets.
    #[test]
    fn a_collapsed_node_takes_its_whole_subtree_with_it() {
        let items = "Root\n  Child\n    Grandchild\n  Sibling\nOther";
        let open = layout(&tree(items), rect());
        assert_eq!(open.len(), 5, "empty CollapsedNodes ⇒ the whole tree");

        let mut c = tree(items);
        c.set_prop("CollapsedNodes", PropValue::String("Child".into()));
        let shut = layout(&c, rect());
        assert_eq!(
            shut.iter().map(|r| r.text.as_str()).collect::<Vec<_>>(),
            vec!["Root", "Child", "Sibling", "Other"],
            "Child stays, its grandchild goes, its sibling is untouched"
        );

        // Folding the ROOT takes everything under it, at every depth.
        c.set_prop("CollapsedNodes", PropValue::String("Root".into()));
        assert_eq!(
            layout(&c, rect())
                .iter()
                .map(|r| r.text.as_str())
                .collect::<Vec<_>>(),
            vec!["Root", "Other"]
        );
    }

    /// Only a node with something under it gets an arrow, and its direction is
    /// its state. A leaf with an arrow would promise a fold that does nothing.
    #[test]
    fn only_a_node_with_children_gets_an_arrow() {
        let mut c = tree("Root\n  Leaf\nOther");
        let rows = layout(&c, rect());
        assert!(rows[0].has_children && rows[0].expander.is_some(), "Root folds");
        assert!(!rows[1].has_children && rows[1].expander.is_none(), "a leaf does not");
        assert!(!rows[2].has_children && rows[2].expander.is_none());

        // …and the labels still line up, because the SLOT is reserved on every
        // row even where no arrow is drawn.
        assert_eq!(
            rows[0].label_x, rows[2].label_x,
            "two roots, one with children and one without, must share a column"
        );

        c.set_prop("CollapsedNodes", PropValue::String("Root".into()));
        assert!(layout(&c, rect())[0].collapsed, "and it knows it is shut");
    }

    /// A node names its own icon after a TAB — the same TAB-separated shape the
    /// Markers, Routes and Regions collections use. Without one it takes the
    /// control's default: a folder for something that holds nodes (open or shut
    /// to match its own arrow) and a document for a leaf.
    #[test]
    fn a_node_names_its_icon_after_a_tab_or_takes_the_default() {
        let rows = layout(&tree("Warehouse\tbox\n  Bolts\nPlain"), rect());
        assert_eq!(rows[0].icon.as_deref(), Some("box"), "the name after the TAB");
        assert_eq!(rows[0].text, "Warehouse", "…and it is NOT part of the label");
        assert_eq!(rows[1].icon.as_deref(), Some("doc-text"), "a leaf is a document");
        assert_eq!(rows[2].icon.as_deref(), Some("doc-text"));

        // An open parent and a shut one are different pictures.
        let mut c = tree("Warehouse\n  Bolts");
        assert_eq!(layout(&c, rect())[0].icon.as_deref(), Some("folder-open"));
        c.set_prop("CollapsedNodes", PropValue::String("Warehouse".into()));
        assert_eq!(layout(&c, rect())[0].icon.as_deref(), Some("folder"));

        // Every icon named here is one the platform actually draws — a default
        // pointing at nothing would be a blank column nobody could explain.
        for name in ["folder", "folder-open", "doc-text", "chevron-right", "chevron-down"] {
            assert!(
                crate::icons::menu_icon_names().any(|n| n == name),
                "{name} is not in the icon catalogue"
            );
        }
    }

    /// `ShowIcons` off is a tree of plain text, and the labels move left to
    /// take the room back.
    #[test]
    fn icons_can_be_turned_off_and_give_their_room_back() {
        let with = layout(&tree("Root\n  Leaf"), rect());
        let mut c = tree("Root\n  Leaf");
        c.set_prop("ShowIcons", PropValue::Bool(false));
        let without = layout(&c, rect());
        assert!(with[0].icon_rect.is_some() && without[0].icon_rect.is_none());
        assert!(
            without[0].label_x < with[0].label_x,
            "the label must reclaim the icon's room"
        );
    }

    /// **Every metric is a property.** A row's height, its indent, its tick box
    /// and its icon were all fixed numbers; a tree on a 24pt font needs taller
    /// rows and nobody could ask for them.
    #[test]
    fn the_metrics_are_properties_and_move_the_layout() {
        let base = layout(&tree("Root\n  Child"), rect());
        let mut c = tree("Root\n  Child");
        c.set_prop("RowHeight", PropValue::Int(40));
        c.set_prop("IndentWidth", PropValue::Int(48));
        let big = layout(&c, rect());
        assert!(
            big[0].rect.height() > base[0].rect.height(),
            "RowHeight must reach the band"
        );
        assert!(
            big[1].rect.center().y - big[0].rect.center().y > 30.0,
            "…and the pitch between rows"
        );
        assert!(
            (big[1].label_x - big[0].label_x) > (base[1].label_x - base[0].label_x),
            "IndentWidth must reach the indent"
        );
    }

    /// **A connector never crosses what the row draws.** The elbow used to run
    /// to the LABEL, and the icon sits between the two — so every line was
    /// drawn straight through its own node's picture (operator, 2026-08-22).
    #[test]
    fn a_connector_stops_before_the_rows_own_drawing() {
        let rows = layout(&tree("Root\n  Child"), rect());
        let child = &rows[1];
        let edge = row_left_edge(child);
        for part in [child.expander, child.check, child.icon_rect]
            .into_iter()
            .flatten()
        {
            assert!(
                edge <= part.min.x,
                "the line must stop left of everything the row draws: {edge} vs {part:?}"
            );
        }
        assert!(edge < child.label_x, "…and left of the label");
    }

    /// **A row is never shorter than what it holds.** Growing the icon used to
    /// leave the pitch alone, so a big icon painted over the nodes above and
    /// below it. `NodeSpacing` then adds a gap on top of that.
    #[test]
    fn a_bigger_icon_makes_a_taller_row_and_spacing_adds_more() {
        let base = TreeMetrics::of(&tree("A")).row_h;
        let mut c = tree("A");
        c.set_prop("IconSize", PropValue::Int(48));
        let tall = TreeMetrics::of(&c).row_h;
        assert!(
            tall >= 48.0,
            "a 48pt icon cannot live in a {tall}pt row without spilling"
        );
        assert!(tall > base);

        c.set_prop("NodeSpacing", PropValue::Int(10));
        assert!(
            (TreeMetrics::of(&c).row_h - (tall + 10.0)).abs() < 0.01,
            "NodeSpacing is a gap ON TOP of what the row needs"
        );
    }

    /// A tree taller than its control stops at the bottom edge rather than
    /// painting nodes outside the control the developer sized.
    #[test]
    fn rows_stop_at_the_bottom_edge() {
        let items = (1..=40)
            .map(|i| format!("Node {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let rows = layout(&tree(&items), rect());
        assert!(rows.len() < 40, "40 rows cannot fit 160pt");
        assert!(
            rows.iter().all(|r| r.rect.max.y <= rect().max.y + ROW_H),
            "no row may hang below the control"
        );
    }

    /// A tree taller than its control SCROLLS instead of dropping the overflow
    /// on the floor. The nodes were always there; there was no way to reach
    /// them.
    #[test]
    fn a_tree_taller_than_its_control_scrolls() {
        let items = (1..=40)
            .map(|i| format!("Node {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let c = tree(&items);
        let max = max_scroll(&c, rect());
        assert!(max > 0.0, "40 rows in 160pt must be scrollable");

        let top = layout_at(&c, rect(), 0.0);
        assert_eq!(top[0].text, "Node 1", "unscrolled starts at the first node");

        let bottom = layout_at(&c, rect(), max);
        assert_eq!(
            bottom.last().expect("rows").text,
            "Node 40",
            "scrolled to the end, the LAST node is reachable — it never was"
        );
        assert!(
            !bottom.iter().any(|r| r.text == "Node 1"),
            "and the head has scrolled away"
        );
    }

    /// The offset is held inside what there is to scroll, at both ends.
    #[test]
    fn scrolling_stops_at_both_ends() {
        let items = (1..=40)
            .map(|i| format!("Node {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let c = tree(&items);
        let max = max_scroll(&c, rect());
        assert_eq!(clamp_scroll(&c, rect(), max + 5_000.0), max, "stops at the end");
        assert_eq!(clamp_scroll(&c, rect(), -500.0), 0.0, "and at the start");

        // A tree that already fits cannot scroll at all, so a stray wheel over
        // it does nothing rather than shifting it by a pixel.
        let short = tree("A\nB");
        assert_eq!(max_scroll(&short, rect()), 0.0);
        assert_eq!(clamp_scroll(&short, rect(), 900.0), 0.0);
    }

    /// How far a tree can scroll is measured against the rows it SHOWS. A tree
    /// folded down to three rows must not scroll as if it held three hundred.
    #[test]
    fn folding_a_tree_shortens_what_there_is_to_scroll() {
        let items = (1..=12)
            .map(|i| format!("Parent {i}\n  Child {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let mut c = tree(&items);
        let open = content_height(&c);
        let folded_list = (1..=12)
            .map(|i| format!("Parent {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        c.set_prop("CollapsedNodes", PropValue::String(folded_list));
        let shut = content_height(&c);
        assert!(
            shut < open,
            "folding every parent halves the tree: {open} → {shut}"
        );
        assert!(
            (shut - (FIRST_ROW_Y + 12.0 * TreeMetrics::of(&c).row_h)).abs() < 0.01,
            "twelve rows remain, not twenty-four: {shut}"
        );
    }

    /// A row scrolled half off the edge is KEPT — that is what makes the tree
    /// slide rather than jump a whole row at a time. The painter clips it.
    #[test]
    fn a_straddling_row_is_kept_not_dropped() {
        let items = (1..=40)
            .map(|i| format!("Node {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let c = tree(&items);
        // Scrolled until the first row's CENTRE sits on the top edge: half of
        // it is above the control and half below.
        let rows = layout_at(&c, rect(), FIRST_ROW_Y);
        let first = &rows[0];
        assert!(
            first.rect.min.y < rect().min.y && first.rect.max.y > rect().min.y,
            "the top row straddles the edge: {:?}",
            first.rect
        );
    }

    /// The view follows a keyboard selection only as far as it must — and not
    /// at all when the row is already on screen.
    #[test]
    fn the_view_follows_the_selection_only_when_it_has_to() {
        let items = (1..=40)
            .map(|i| format!("Node {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let c = tree(&items);
        assert_eq!(
            scroll_to_row(&c, rect(), 1, 0.0),
            0.0,
            "row 1 is already in view: the tree must not move"
        );
        let deep = scroll_to_row(&c, rect(), 39, 0.0);
        assert!(deep > 0.0, "the last row is not: the view follows");
        let rows = layout_at(&c, rect(), deep);
        assert!(
            rows.iter().any(|r| r.text == "Node 40"),
            "and lands with it visible: {:?}",
            rows.iter().map(|r| &r.text).collect::<Vec<_>>()
        );
        assert_eq!(
            scroll_to_row(&c, rect(), 39, deep),
            deep,
            "asked again, it stays put"
        );
    }

    /// `_DeferTree` stops the FACE painter drawing a tree, so the running form
    /// — which paints its own, scrolled and hot-tracked — gets exactly one.
    ///
    /// Both drew until 1.61.160, harmlessly, because both sat at scroll 0 and
    /// landed on top of each other. The moment one of them could scroll, every
    /// scroll smeared a ghost copy of the tree across the live one (operator,
    /// 2026-08-22). The bug was invisible for as long as the two agreed, which
    /// is why this asserts the FACE draws no nodes rather than asserting the
    /// pixels match.
    #[test]
    fn the_face_painter_defers_the_tree_when_asked() {
        let ctx = egui::Context::default();
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(600.0, 400.0)));

        let draw = |ctrl: &Control| -> usize {
            let mut full = ctx.clone().run_ui(input.clone(), |ui| {
                crate::paint::draw_control(
                    ui.painter(),
                    Pos2::ZERO,
                    ctrl,
                    false,
                    true,
                    1.0,
                    1.0,
                    None,
                );
            });
            full.textures_delta.clear();
            fn count(s: &egui::Shape) -> usize {
                match s {
                    egui::Shape::Vec(v) => v.iter().map(count).sum(),
                    egui::Shape::Text(_) => 1,
                    _ => 0,
                }
            }
            full.shapes.iter().map(|cs| count(&cs.shape)).sum()
        };

        let plain = tree("Node 1\n  Child 1\nNode 2");
        let mut deferred = plain.clone();
        deferred.set_prop("_DeferTree", PropValue::Bool(true));

        let with_tree = draw(&plain);
        let face_only = draw(&deferred);
        assert!(
            with_tree > face_only,
            "the face painter must draw the nodes when it is not deferred: \
             {with_tree} vs {face_only}"
        );
        assert_eq!(
            face_only, 0,
            "and none of them when it is — every node it draws is a second, \
             unscrolled copy under the running form's own"
        );
    }

    /// Everything one `paint` pass put on the screen, flattened — the same
    /// shape-walk the CheckBox's own surface tests use.
    #[derive(Default)]
    struct Painted {
        fills: Vec<(Color32, Rect)>,
        strokes: Vec<(Color32, f32)>,
    }

    impl Painted {
        fn fill_near(&self, rgb: (u8, u8, u8), tol: i32) -> Option<Rect> {
            self.fills
                .iter()
                .find(|(c, _)| {
                    (c.r() as i32 - rgb.0 as i32).abs() <= tol
                        && (c.g() as i32 - rgb.1 as i32).abs() <= tol
                        && (c.b() as i32 - rgb.2 as i32).abs() <= tol
                })
                .map(|(_, r)| *r)
        }
        fn has_stroke(&self, rgb: (u8, u8, u8)) -> bool {
            self.strokes
                .iter()
                .any(|(c, _)| (c.r(), c.g(), c.b()) == rgb)
        }
        fn widest_stroke(&self, rgb: (u8, u8, u8)) -> f32 {
            self.strokes
                .iter()
                .filter(|(c, _)| (c.r(), c.g(), c.b()) == rgb)
                .map(|(_, w)| *w)
                .fold(0.0, f32::max)
        }
    }

    fn painted(ctrl: &Control, checked: &[String]) -> Painted {
        let ctx = egui::Context::default();
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(600.0, 400.0)));
        let rows = layout(ctrl, rect());
        let mut full = ctx.run_ui(input, |ui| {
            paint(
                ui.painter(),
                ctrl,
                rect(),
                &rows,
                TreeState {
                    selected: "",
                    checked,
                    hovered: None,
                    alpha: 1.0,
                },
            );
        });
        full.textures_delta.clear();
        fn walk(s: &egui::Shape, out: &mut Painted) {
            match s {
                egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
                egui::Shape::Rect(r) => {
                    if r.fill.a() > 0 {
                        out.fills.push((r.fill, r.rect));
                    }
                    if r.stroke.width > 0.0 && r.stroke.color.a() > 0 {
                        out.strokes.push((r.stroke.color, r.stroke.width));
                    }
                }
                egui::Shape::LineSegment { stroke, .. } if stroke.width > 0.0 => {
                    out.strokes.push((stroke.color, stroke.width));
                }
                _ => {}
            }
        }
        let mut seen = Painted::default();
        for cs in &full.shapes {
            walk(&cs.shape, &mut seen);
        }
        seen
    }

    fn checkable() -> Control {
        let mut c = tree("Alpha\nBeta");
        c.set_prop("CheckBoxes", PropValue::Bool(true));
        // Off, so the tree's own lines cannot be mistaken for the box's rim.
        c.set_prop("ShowLines", PropValue::Bool(false));
        c.set_prop("ShowRootLines", PropValue::Bool(false));
        c.set_prop("ShowIcons", PropValue::Bool(false));
        c
    }

    /// The tick box wears the five properties a CheckBox wears. Before this the
    /// box was a black-alpha well, a 1px rim in the node ink and a tick at 28 %
    /// of the box — three numbers nailed into the painter, none reachable.
    #[test]
    fn the_tick_box_wears_the_checkbox_family() {
        let mut c = checkable();
        c.set_prop("CheckBoxColor", PropValue::String("#C81E1E".into()));
        c.set_prop("CheckBoxBorderColor", PropValue::String("#00A000".into()));
        c.set_prop("CheckBoxBorderWidth", PropValue::Int(3));
        c.set_prop("CheckColor", PropValue::String("#1E3CC8".into()));

        let seen = painted(&c, &["Alpha".to_string()]);
        let box_rect = seen
            .fill_near((0xC8, 0x1E, 0x1E), 6)
            .expect("CheckBoxColor must paint the tick box");
        assert!(
            box_rect.width() < 40.0,
            "and it is the BOX that wears it, not the band: {box_rect:?}"
        );
        assert!(
            seen.has_stroke((0x00, 0xA0, 0x00)),
            "CheckBoxBorderColor must rim the box"
        );
        assert!(
            seen.has_stroke((0x1E, 0x3C, 0xC8)),
            "CheckColor must draw the tick of a ticked node"
        );
    }

    /// `None` switches the rim off — the reason a style property was added
    /// beside the colour, exactly as the frame's own `BorderStyle` was.
    #[test]
    fn a_none_border_leaves_the_box_unrimmed() {
        let mut c = checkable();
        c.set_prop("CheckBoxBorderColor", PropValue::String("#00A000".into()));
        assert!(
            painted(&c, &[]).has_stroke((0x00, 0xA0, 0x00)),
            "seeded Single: the box is rimmed"
        );
        c.set_prop("CheckBoxBorderStyle", PropValue::String("None".into()));
        assert!(
            !painted(&c, &[]).has_stroke((0x00, 0xA0, 0x00)),
            "None must leave the box unrimmed"
        );
    }

    /// `CheckSize` is the TICK's share of the box, `CheckBoxSize` is the box —
    /// the same split a CheckBox makes. A bigger share draws a heavier mark.
    #[test]
    fn check_size_scales_the_tick_not_the_box() {
        let mut c = checkable();
        c.set_prop("CheckColor", PropValue::String("#1E3CC8".into()));
        c.set_prop("CheckBoxSize", PropValue::Int(32));
        c.set_prop("CheckSize", PropValue::Int(20));
        let small = painted(&c, &["Alpha".to_string()]).widest_stroke((0x1E, 0x3C, 0xC8));
        c.set_prop("CheckSize", PropValue::Int(100));
        let big = painted(&c, &["Alpha".to_string()]).widest_stroke((0x1E, 0x3C, 0xC8));
        assert!(
            big > small,
            "a fuller tick draws a heavier mark: {small} → {big}"
        );

        // And the BOX is unmoved by it — that is `CheckBoxSize`'s job.
        let rows = layout(&c, rect());
        let box_w = rows[0].check.expect("checkboxes are on").width();
        assert!(
            (box_w - 32.0).abs() < 0.01,
            "CheckBoxSize sizes the box: {box_w}"
        );
    }
}
