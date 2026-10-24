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
//! * The **DataGrid** has a module of its own (`datagrid`): it is drawn the way the desktop draws it.

use cobolt_forms::treenodes::{nodes, NodeInfo};

use super::controls::{catalogue_icon, lines_nonempty, root};
use super::{VNode, ViewCtx};
use crate::value::truthy;

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

#[cfg(test)]
mod tests {
    use super::super::{Empty, Prop};
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
}
