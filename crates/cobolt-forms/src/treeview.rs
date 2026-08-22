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
        Self {
            row_h: num(ctrl, "RowHeight", ROW_H, 8.0, 200.0),
            indent: num(ctrl, "IndentWidth", INDENT, 0.0, 200.0),
            check: num(ctrl, "CheckBoxSize", CHECK, 6.0, 64.0),
            icon: num(ctrl, "IconSize", ICON, 6.0, 64.0),
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

/// One parsed line: `(index, depth, label, icon)`.
type ParsedNode = (usize, usize, String, Option<String>);

/// `Items` parsed, blank lines dropped.
///
/// A line is `label`, optionally followed by a TAB and the name of an icon from
/// the platform's catalogue — the same TAB-separated shape the Markers, Routes
/// and Regions collections use, so a developer meets one convention rather than
/// four. `Warehouse\tfolder` is a node with a folder on it.
fn parse(items: &str) -> Vec<ParsedNode> {
    items
        .lines()
        .enumerate()
        .filter_map(|(i, line)| {
            // Two spaces per level, the shape the inspector's own hint teaches.
            // A LEADING tab counts as one level so a developer who typed one is
            // not punished for it; a tab after the label names the icon.
            let lead_tabs = line.chars().take_while(|c| *c == '\t').count();
            let body = &line[lead_tabs..];
            let indent = body.len() - body.trim_start().len();
            let depth = if lead_tabs > 0 { lead_tabs } else { indent / 2 };
            let mut parts = body.trim_start().splitn(2, '\t');
            let text = parts.next().unwrap_or("").trim();
            if text.is_empty() {
                return None;
            }
            let icon = parts
                .next()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_owned);
            Some((i, depth, text.to_owned(), icon))
        })
        .collect()
}

/// Sort SIBLINGS, leaving every node under the parent it was written under.
///
/// A flat `sort_by` would tear children away from their parents — the nodes
/// would be in order and the tree would be nonsense.
fn sort_siblings(nodes: Vec<ParsedNode>) -> Vec<ParsedNode> {
    // Each node's own subtree is the run of following nodes deeper than it.
    fn walk(nodes: &[ParsedNode], depth: usize) -> Vec<ParsedNode> {
        let mut groups: Vec<Vec<ParsedNode>> = Vec::new();
        for node in nodes {
            if node.1 <= depth || groups.is_empty() {
                groups.push(vec![node.clone()]);
            } else {
                groups.last_mut().expect("just pushed").push(node.clone());
            }
        }
        groups.sort_by(|a, b| a[0].2.to_lowercase().cmp(&b[0].2.to_lowercase()));
        let mut out = Vec::new();
        for g in groups {
            out.push(g[0].clone());
            if g.len() > 1 {
                out.extend(walk(&g[1..], depth + 1));
            }
        }
        out
    }
    let base = nodes.first().map(|n| n.1).unwrap_or(0);
    walk(&nodes, base)
}

fn flag(ctrl: &Control, key: &str, default: bool) -> bool {
    ctrl.get_prop(key).map(|v| v.as_bool()).unwrap_or(default)
}

/// A band colour the developer named, used EXACTLY as given — alpha included,
/// since a selection band is mostly alpha. `None` when they named none.
fn band_color(ctrl: &Control, key: &str) -> Option<Color32> {
    ctrl.get_prop(key)
        .map(|v| v.as_str().trim().to_owned())
        .filter(|s| !s.is_empty())
        .map(|s| crate::paint::parse_color(&s))
        .filter(|c| c.a() > 0)
}

/// Lay the tree out inside `rect`. Rows past the bottom edge are dropped: a
/// node drawn outside its own control is worse than one the operator scrolls
/// to, and the control does not scroll yet.
pub fn layout(ctrl: &Control, rect: Rect) -> Vec<TreeRow> {
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
    let m = TreeMetrics::of(ctrl);
    let checks = flag(ctrl, "CheckBoxes", false);
    let icons = flag(ctrl, "ShowIcons", true);
    let mut y = rect.min.y + FIRST_ROW_Y;
    let mut rows = Vec::new();
    // Everything deeper than this is inside something folded shut.
    let mut hide_below: Option<usize> = None;
    for (n, (index, depth, text, icon)) in nodes.iter().enumerate() {
        let (index, depth, text) = (*index, *depth, text.clone());
        match hide_below {
            Some(d) if depth > d => continue,
            Some(_) => hide_below = None,
            None => {}
        }
        // A node HAS children when the next line is deeper than it — the only
        // thing that earns a disclosure arrow.
        let has_children = nodes.get(n + 1).map(|next| next.1 > depth).unwrap_or(false);
        let is_collapsed = has_children && collapsed.iter().any(|c| *c == text);
        if is_collapsed {
            hide_below = Some(depth);
        }
        if y + m.row_h * 0.5 > rect.max.y {
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
            let left = row.check.map(|c| c.min.x).unwrap_or(row.label_x) - 4.0;
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
                        Pos2::new(r.check.map(|c| c.min.x).unwrap_or(r.label_x) - 4.0, r.rect.center().y),
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
        if selected {
            // `SelectionColor` when the developer named one — used exactly as
            // given, alpha and all. Empty keeps the theme's focus colour at the
            // weight a selection band has always had.
            let band = band_color(ctrl, "SelectionColor").unwrap_or_else(|| {
                let c = focus.unwrap_or(Color32::from_rgb(70, 110, 200));
                Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), 70)
            });
            painter.rect_filled(row.rect, 3.0, fade(band));
        } else if state.hovered == Some(row.index) {
            // HotTracking: the row under the pointer lifts, faintly — half the
            // selection's weight by default, so the two are never confused.
            let band = band_color(ctrl, "HotTrackColor")
                .unwrap_or(Color32::from_white_alpha(18));
            painter.rect_filled(row.rect, 3.0, fade(band));
        }
        if let Some(box_rect) = row.check {
            let ticked = state.checked.iter().any(|c| c == &row.text);
            painter.rect_filled(box_rect, 2.0, fade(Color32::from_black_alpha(40)));
            painter.rect_stroke(
                box_rect,
                2.0,
                Stroke::new(1.0, fade(ink)),
                egui::StrokeKind::Inside,
            );
            if ticked {
                let c = box_rect.center();
                let s = box_rect.width() * 0.28;
                let stroke = Stroke::new(2.0, fade(ink));
                painter.line_segment(
                    [Pos2::new(c.x - s, c.y), Pos2::new(c.x - s * 0.2, c.y + s)],
                    stroke,
                );
                painter.line_segment(
                    [Pos2::new(c.x - s * 0.2, c.y + s), Pos2::new(c.x + s, c.y - s)],
                    stroke,
                );
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
        painter.text(
            Pos2::new(row.label_x, row.rect.center().y),
            egui::Align2::LEFT_CENTER,
            &row.text,
            font.clone(),
            fade(ink),
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
}
