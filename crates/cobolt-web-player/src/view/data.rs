// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The data controls as elements**: TreeView and DataGrid (WASM-004 class A, with the DataGrid's
//! backend-bound half in class C).
//!
//! * The **TreeView** is an ARIA tree of nested lists built from `Items` the way the desktop reads it
//!   (`treenodes::nodes`): two spaces of indentation per level, an optional icon after a TAB.
//! * The **DataGrid** is a `<table>` that **draws only the rows in view** (plus an overscan): a grid of
//!   a million rows is a few dozen `<tr>`. Sorting by a header is a *view* sort, as on the desktop —
//!   the `Rows` property is not reordered by the user's click (the `Sort` method does that), and
//!   `ClickedRow` always names the row's number in `Rows`.

use cobolt_forms::datagrid::parse_rows;
use cobolt_forms::treenodes::{nodes, NodeInfo};

use super::controls::{catalogue_icon, lines_nonempty, root};
use super::{VNode, ViewCtx};
use crate::value::{fmt_number, parse_f64, truthy};

// ── TreeView ────────────────────────────────────────────────────────────────

/// The payload a tree event carries: `text⇥index⇥level⇥checked` (`render::node_payload`).
pub fn node_payload(n: &NodeInfo, checked: &[String]) -> String {
    format!("{}\t{}\t{}\t{}", n.text, n.index, n.level, if checked.iter().any(|c| *c == n.text) { 1 } else { 0 })
}

/// Which nodes the tree shows: a node whose ancestor is folded shut is not.
pub fn visible_nodes<'a>(all: &'a [NodeInfo], collapsed: &[String]) -> Vec<&'a NodeInfo> {
    let mut out = Vec::new();
    let mut hide_below: Option<usize> = None;
    for n in all {
        if let Some(level) = hide_below {
            if n.level > level {
                continue;
            }
            hide_below = None;
        }
        out.push(n);
        if n.child_count > 0 && collapsed.iter().any(|c| *c == n.text) {
            hide_below = Some(n.level);
        }
    }
    out
}

pub fn tree_view(cx: &ViewCtx, i: usize) -> VNode {
    let items = cx.tree.get(i, "Items");
    let all = nodes(&items);
    let selected = cx.tree.get(i, "SelectedNode");
    let checked = lines_nonempty(&cx.tree.get(i, "CheckedNodes").lines().map(str::trim).collect::<Vec<_>>().join("\n"));
    let collapsed: Vec<String> = cx.tree.get(i, "CollapsedNodes").lines().map(str::trim).filter(|s| !s.is_empty()).map(str::to_owned).collect();
    let checkboxes = truthy(&cx.tree.get(i, "CheckBoxes"));
    let show_icons = truthy(&cx.tree.get(i, "ShowIcons"));
    let (parent_icon, parent_open, leaf_icon) = (cx.tree.get(i, "ParentIcon"), cx.tree.get(i, "ParentIconOpen"), cx.tree.get(i, "LeafIcon"));
    let mut sorted_levels = false;
    if truthy(&cx.tree.get(i, "Sorted")) {
        sorted_levels = true;
    }
    let _ = sorted_levels;

    // Build the nested lists from the flat, depth-annotated nodes.
    fn build(
        all: &[&NodeInfo],
        at: &mut usize,
        level: usize,
        selected: &str,
        checked: &[String],
        collapsed: &[String],
        checkboxes: bool,
        show_icons: bool,
        icons: (&str, &str, &str),
        node_icon: &dyn Fn(&str) -> Option<VNode>,
    ) -> Vec<VNode> {
        let mut out = Vec::new();
        while *at < all.len() {
            let n = all[*at];
            if n.level < level {
                break;
            }
            if n.level > level {
                // Uneven indentation: treat a too-deep node as a child of the previous one.
                break;
            }
            *at += 1;
            let has_children = n.child_count > 0;
            let is_collapsed = has_children && collapsed.iter().any(|c| *c == n.text);
            let is_selected = !selected.is_empty() && selected == n.text;
            let mut row = VNode::new("div").key(format!("row:{}", n.index)).class("pr-node-row");
            if has_children {
                row = row.child(
                    VNode::new("button")
                        .key(format!("arrow:{}", n.index))
                        .class("pr-arrow")
                        .attr("type", "button")
                        .attr("tabindex", "-1")
                        .attr("data-part", "arrow")
                        .attr("aria-label", if is_collapsed { "Expand" } else { "Collapse" })
                        .text(if is_collapsed { "\u{25B8}" } else { "\u{25BE}" }),
                );
            } else {
                row = row.child(VNode::new("span").key(format!("sp:{}", n.index)).class("pr-arrow pr-arrow-none"));
            }
            if checkboxes {
                row = row.child(
                    VNode::new("input")
                        .key(format!("chk:{}", n.index))
                        .attr("type", "checkbox")
                        .attr("tabindex", "-1")
                        .attr("data-part", "check")
                        .prop_bool("checked", checked.iter().any(|c| *c == n.text)),
                );
            }
            if show_icons {
                let name = n.icon.clone().filter(|s| !s.trim().is_empty()).unwrap_or_else(|| {
                    if has_children {
                        if is_collapsed { icons.0.to_owned() } else { icons.1.to_owned() }
                    } else {
                        icons.2.to_owned()
                    }
                });
                if let Some(svg) = node_icon(&name) {
                    row = row.child(svg.key(format!("ico:{}", n.index)).class("pr-node-icon"));
                }
            }
            row = row.child(VNode::new("span").key(format!("txt:{}", n.index)).class("pr-node-text").attr("data-part", "label").text(n.text.clone()));
            let mut li = VNode::new("li")
                .key(format!("node:{}", n.index))
                .attr("role", "treeitem")
                .attr("data-i", n.index.to_string())
                .attr("aria-level", (n.level + 1).to_string())
                .attr("aria-selected", if is_selected { "true" } else { "false" })
                .child(row);
            if has_children {
                li = li.attr("aria-expanded", if is_collapsed { "false" } else { "true" });
                if !is_collapsed {
                    let kids = build(all, at, level + 1, selected, checked, collapsed, checkboxes, show_icons, icons, node_icon);
                    if !kids.is_empty() {
                        li = li.child(VNode::new("ul").key(format!("grp:{}", n.index)).attr("role", "group").children(kids));
                    }
                } else {
                    // Skip the folded children.
                    while *at < all.len() && all[*at].level > n.level {
                        *at += 1;
                    }
                }
            }
            if is_selected {
                li = li.class("pr-selected");
            }
            out.push(li);
        }
        out
    }

    let visible = visible_nodes(&all, &collapsed);
    // `visible_nodes` already removed the folded subtrees, so `build` sees an unfolded list but must
    // still draw the arrow state: it consults `collapsed` for the arrow of a node that has children.
    let mut at = 0usize;
    let node_icon = |name: &str| catalogue_icon(name);
    let min_level = visible.iter().map(|n| n.level).min().unwrap_or(0);
    let mut top: Vec<VNode> = Vec::new();
    while at < visible.len() {
        let before = at;
        let mut chunk = build(&visible, &mut at, min_level, &selected, &checked, &collapsed, checkboxes, show_icons, (&parent_icon, &parent_open, &leaf_icon), &node_icon);
        top.append(&mut chunk);
        if at == before {
            at += 1; // a node deeper than any level the walk accepts: never loop on it
        }
    }
    let mut root_n = root(cx, i, "ul").attr("role", "tree").attr("tabindex", "0").on("click").on("dblclick").on("keydown");
    root_n.children = top;
    root_n
}

// ── DataGrid ────────────────────────────────────────────────────────────────

/// One column: the title and what kind of value it holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Column {
    pub title: String,
    pub numeric: bool,
}

/// `Columns` is one `Name:Type` per line (type `string`, `number` or `datetime`).
pub fn columns(text: &str) -> Vec<Column> {
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let (name, ty) = l.split_once(':').unwrap_or((l, "string"));
            Column { title: name.trim().to_owned(), numeric: ty.trim().eq_ignore_ascii_case("number") }
        })
        .collect()
}

/// How many rows are drawn before and after the ones in view.
pub const OVERSCAN: usize = 12;
/// The rows drawn when nothing has measured the grid yet.
pub const DEFAULT_WINDOW: usize = 40;

/// The order the user's sort puts the rows in: indexes into `rows`, stable.
pub fn sorted_order(rows: &[Vec<String>], column: usize, descending: bool) -> Vec<usize> {
    let mut order: Vec<usize> = (0..rows.len()).collect();
    let cmp = |a: &usize, b: &usize| {
        let x = rows[*a].get(column).map(String::as_str).unwrap_or("");
        let y = rows[*b].get(column).map(String::as_str).unwrap_or("");
        match (parse_f64(x), parse_f64(y)) {
            (Some(p), Some(q)) => p.partial_cmp(&q).unwrap_or(std::cmp::Ordering::Equal),
            _ => x.to_lowercase().cmp(&y.to_lowercase()),
        }
    };
    if descending {
        order.sort_by(|a, b| cmp(b, a));
    } else {
        order.sort_by(cmp);
    }
    order
}

pub fn row_height(cx: &ViewCtx, i: usize) -> f64 {
    parse_f64(&cx.tree.get(i, "RowHeight")).filter(|h| *h >= 14.0).unwrap_or(22.0).min(400.0)
}

pub fn data_grid(cx: &ViewCtx, i: usize) -> VNode {
    let cols = columns(&cx.tree.get(i, "Columns"));
    let rows = parse_rows(&cx.tree.get(i, "Rows"));
    let ncols = cols.len().max(rows.iter().map(Vec::len).max().unwrap_or(0));
    let rh = row_height(cx, i);
    let sort = cx.ui.grid_sort.get(&i).copied();
    let order: Vec<usize> = match sort {
        Some((c, desc)) if truthy(&cx.tree.get(i, "AllowSorting")) => sorted_order(&rows, c, desc),
        _ => (0..rows.len()).collect(),
    };
    let (first, count) = cx.ui.grid_window.get(&i).copied().unwrap_or((0, DEFAULT_WINDOW));
    let lo = first.saturating_sub(OVERSCAN).min(order.len());
    let hi = (first + count + OVERSCAN).min(order.len());
    let clicked = parse_f64(&cx.tree.get(i, "ClickedRow")).map(|v| v as i64).unwrap_or(0);
    let numbers = truthy(&cx.tree.get(i, "ShowRowNumbers"));
    let mut head = VNode::new("tr").key("head");
    if numbers {
        head = head.child(VNode::new("th").key("h:#").class("pr-rownum").attr("scope", "col").text("#"));
    }
    for c in 0..ncols {
        let title = cols.get(c).map(|c| c.title.clone()).unwrap_or_else(|| format!("{}", c + 1));
        let mut th = VNode::new("th").key(format!("h:{c}")).attr("scope", "col").attr("data-c", c.to_string()).text(title);
        if let Some((sc, desc)) = sort {
            if sc == c {
                th = th.attr("aria-sort", if desc { "descending" } else { "ascending" });
            }
        }
        head = head.child(th);
    }
    let mut body = VNode::new("tbody").key("body");
    let span = ncols + usize::from(numbers);
    if lo > 0 {
        body = body.child(VNode::new("tr").key("pad-top").class("pr-pad").attr("aria-hidden", "true").style("height", format!("{}px", fmt_number(lo as f64 * rh))).child(
            VNode::new("td").key("pad-top-c").attr("colspan", span.to_string()),
        ));
    }
    for (view_pos, &orig) in order.iter().enumerate().take(hi).skip(lo) {
        let row = &rows[orig];
        let mut tr = VNode::new("tr")
            .key(format!("row:{orig}"))
            .attr("data-r", orig.to_string())
            .attr("aria-rowindex", (view_pos + 2).to_string())
            .attr("aria-selected", if clicked == orig as i64 + 1 { "true" } else { "false" })
            .style("height", format!("{}px", fmt_number(rh)));
        if numbers {
            tr = tr.child(VNode::new("td").key(format!("n:{orig}")).class("pr-rownum").text((orig + 1).to_string()));
        }
        for c in 0..ncols {
            let mut td = VNode::new("td").key(format!("c:{orig}:{c}")).attr("data-c", c.to_string()).text(row.get(c).cloned().unwrap_or_default());
            if cols.get(c).is_some_and(|c| c.numeric) {
                td = td.class("pr-num");
            }
            tr = tr.child(td);
        }
        body = body.child(tr);
    }
    if hi < order.len() {
        body = body.child(
            VNode::new("tr")
                .key("pad-bottom")
                .class("pr-pad")
                .attr("aria-hidden", "true")
                .style("height", format!("{}px", fmt_number((order.len() - hi) as f64 * rh)))
                .child(VNode::new("td").key("pad-bottom-c").attr("colspan", span.to_string())),
        );
    }
    let alt = cx.tree.get(i, "AlternatingMode").to_ascii_lowercase();
    let mut n = root(cx, i, "div")
        .attr("role", "grid")
        .attr("aria-rowcount", (rows.len() + 1).to_string())
        .attr("aria-colcount", span.to_string())
        .attr("tabindex", "0")
        .on("click")
        .on("dblclick")
        .on("keydown")
        .on("scroll");
    match alt.as_str() {
        "rows" => n = n.class("pr-zebra-rows"),
        "columns" => n = n.class("pr-zebra-columns"),
        _ => {}
    }
    n.child(
        VNode::new("table")
            .key("table")
            .attr("role", "presentation")
            .child(VNode::new("thead").key("thead").child(head))
            .child(body),
    )
}

#[cfg(test)]
mod tests {
    use super::super::{Empty, Prop, UiState};
    use super::*;
    use crate::tree::testing::tree_with;

    fn tree_of_items(items: &str, props: &[(&str, &str)]) -> VNode {
        let mut all = vec![("Items", items)];
        all.extend_from_slice(props);
        let (t, i) = tree_with("TreeView", "TRE-1", &all);
        let e = Empty::default();
        let cx = e.cx(&t);
        tree_view(&cx, i)
    }

    fn grid_of(columns: &str, rows: &str, ui: UiState) -> VNode {
        let (t, i) = tree_with("DataGrid", "GRD-1", &[("Columns", columns), ("Rows", rows)]);
        let mut e = Empty::default();
        e.ui = ui;
        let cx = e.cx(&t);
        data_grid(&cx, i)
    }

    #[test]
    fn a_tree_is_nested_lists_with_levels_expanded_state_and_the_selected_node() {
        let n = tree_of_items("Node 1\n  Child 1\n  Child 2\nNode 2", &[("SelectedNode", "Child 2")]);
        assert_eq!(n.attr_of("role"), Some("tree"));
        assert_eq!(n.children.len(), 2, "two roots: {}", n.describe());
        let first = &n.children[0];
        assert_eq!(first.attr_of("aria-expanded"), Some("true"));
        assert_eq!(first.attr_of("aria-level"), Some("1"));
        let kids = first.find_tag("ul").expect("a group");
        assert_eq!(kids.children.len(), 2);
        assert_eq!(kids.children[1].attr_of("aria-selected"), Some("true"));
        assert_eq!(kids.children[1].attr_of("aria-level"), Some("2"));
        assert_eq!(n.children[1].attr_of("aria-expanded"), None, "a leaf has no expanded state");
    }

    #[test]
    fn a_folded_node_hides_its_children_and_says_so() {
        let n = tree_of_items("Node 1\n  Child 1\n  Child 2\nNode 2", &[("CollapsedNodes", "Node 1")]);
        assert_eq!(n.children[0].attr_of("aria-expanded"), Some("false"));
        assert!(n.children[0].find_tag("ul").is_none());
        assert_eq!(n.count("li"), 2, "{}", n.describe());
    }

    #[test]
    fn check_boxes_show_the_ticked_set() {
        let n = tree_of_items("A\n  B\nC", &[("CheckBoxes", "1"), ("CheckedNodes", "B")]);
        let mut boxes = Vec::new();
        n.find_all(&|x| x.tag == "input", &mut boxes);
        let ticked: Vec<bool> = boxes.iter().map(|b| b.prop_of("checked") == Some(&Prop::Bool(true))).collect();
        assert_eq!(ticked, [false, true, false]);
    }

    #[test]
    fn the_node_payload_is_the_one_the_desktop_gives_a_handler() {
        let all = nodes("A\n  B");
        assert_eq!(node_payload(&all[1], &["B".into()]), "B\t1\t1\t1");
    }

    #[test]
    fn a_data_grid_draws_only_the_rows_in_view_and_pads_the_rest() {
        let rows: String = (0..10_000).map(|i| format!("r{i}\t{i}")).collect::<Vec<_>>().join("\n");
        let mut ui = UiState::default();
        ui.grid_window.insert(1, (5000, 30));
        let n = grid_of("Name:string\nValue:number", &rows, ui);
        let trs = n.count("tr");
        assert!(trs < 80, "{trs} rows drawn for 10,000");
        let table = n.find_tag("tbody").unwrap();
        assert_eq!(table.children.first().unwrap().key, "pad-top");
        assert_eq!(table.children.last().unwrap().key, "pad-bottom");
        let top = table.children[0].styles.iter().find(|(k, _)| k == "height").unwrap();
        assert_eq!(top.1, format!("{}px", (5000 - OVERSCAN) * 22));
        assert_eq!(n.attr_of("aria-rowcount"), Some("10001"));
        let numeric = n.find(&|x| x.tag == "td" && x.has_class("pr-num")).expect("a numeric cell");
        assert!(numeric.all_text().chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn a_header_click_sorts_the_view_and_keeps_each_rows_number() {
        let rows = "b\t2\na\t10\nc\t1";
        let mut ui = UiState::default();
        ui.grid_sort.insert(1, (1, false));
        let n = grid_of("Name:string\nValue:number", rows, ui);
        let mut trs = Vec::new();
        n.find_all(&|x| x.tag == "tr" && x.attr_of("data-r").is_some(), &mut trs);
        let order: Vec<&str> = trs.iter().map(|r| r.attr_of("data-r").unwrap()).collect();
        assert_eq!(order, ["2", "0", "1"], "numeric order 1, 2, 10; data-r is the row's number in Rows");
        let sorted = n.find(&|x| x.tag == "th" && x.attr_of("aria-sort").is_some()).unwrap();
        assert_eq!(sorted.attr_of("aria-sort"), Some("ascending"));
        assert_eq!(sorted_order(&[vec!["b".into()], vec!["A".into()]], 0, true), vec![0, 1]);
    }

    #[test]
    fn the_selected_row_is_the_clicked_one() {
        let (t, i) = tree_with("DataGrid", "GRD-1", &[("Columns", "A:string"), ("Rows", "x\ny\nz"), ("ClickedRow", "2")]);
        let e = Empty::default();
        let n = data_grid(&e.cx(&t), i);
        let mut sel = Vec::new();
        n.find_all(&|x| x.attr_of("aria-selected") == Some("true"), &mut sel);
        assert_eq!(sel.len(), 1);
        assert_eq!(sel[0].attr_of("data-r"), Some("1"));
    }

    #[test]
    fn columns_come_from_name_type_lines() {
        let c = columns("Name:string\nQty:number\nWhen:datetime\nPlain");
        assert_eq!(c.len(), 4);
        assert!(c[1].numeric && !c[0].numeric);
        assert_eq!(c[3].title, "Plain");
    }
}
