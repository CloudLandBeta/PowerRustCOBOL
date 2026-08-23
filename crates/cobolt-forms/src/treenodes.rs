// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The TreeView's TREE — `Items` parsed into nodes, and the family links that
//! let a handler walk them.
//!
//! Split out of [`crate::treeview`] and deliberately NOT behind the `render`
//! feature. The renderer needs egui to lay a tree out and paint it; the
//! interpreter needs none of that to answer a handler asking for a node's
//! parent. Both read the tree through this module, so the nodes a handler walks
//! are exactly the nodes the canvas draws — a second parser in the runtime
//! would have drifted from this one the first time either changed.
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
//! A node's index is its line's position in `Items` as WRITTEN — never its
//! position after sorting, and never its position among the nodes that survived
//! a blank line. That index is what an event carries and what every lookup here
//! takes.

/// One parsed line of `Items`.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ParsedNode {
    pub(crate) index: usize,
    pub(crate) depth: usize,
    pub(crate) text: String,
    pub(crate) icon: Option<String>,
    pub(crate) color: Option<String>,
    pub(crate) background: Option<String>,
}

/// `Items` parsed, blank lines dropped.
///
/// A line is `label`, then up to three TAB-separated fields of its own:
///
/// ```text
/// label ⇥ icon ⇥ colour ⇥ background
/// ```
///
/// The same TAB-separated shape the Markers, Routes and Regions collections
/// use, so a developer meets one convention rather than four.
/// `Warehouse⇥folder` is a node with a folder on it; `Overdue⇥⇥#C81E1E` is a
/// node written in red, its icon left to the tree. Every field is optional and
/// an empty one means "as the tree draws it" — the same rule the control's own
/// colour properties follow.
pub(crate) fn parse(items: &str) -> Vec<ParsedNode> {
    items
        .lines()
        .enumerate()
        .filter_map(|(i, line)| {
            // Two spaces per level, the shape the inspector's own hint teaches.
            // A LEADING tab counts as one level so a developer who typed one is
            // not punished for it; the tabs after the label are its fields.
            let lead_tabs = line.chars().take_while(|c| *c == '\t').count();
            let body = &line[lead_tabs..];
            let indent = body.len() - body.trim_start().len();
            let depth = if lead_tabs > 0 { lead_tabs } else { indent / 2 };
            let mut parts = body.trim_start().splitn(4, '\t');
            let text = parts.next().unwrap_or("").trim();
            if text.is_empty() {
                return None;
            }
            let mut field = || {
                parts
                    .next()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned)
            };
            let icon = field();
            let color = field();
            let background = field();
            Some(ParsedNode {
                index: i,
                depth,
                text: text.to_owned(),
                icon,
                color,
                background,
            })
        })
        .collect()
}

/// Sort SIBLINGS, leaving every node under the parent it was written under.
///
/// A flat `sort_by` would tear children away from their parents — the nodes
/// would be in order and the tree would be nonsense.
pub(crate) fn sort_siblings(nodes: Vec<ParsedNode>) -> Vec<ParsedNode> {
    // Each node's own subtree is the run of following nodes deeper than it.
    fn walk(nodes: &[ParsedNode], depth: usize) -> Vec<ParsedNode> {
        let mut groups: Vec<Vec<ParsedNode>> = Vec::new();
        for node in nodes {
            if node.depth <= depth || groups.is_empty() {
                groups.push(vec![node.clone()]);
            } else {
                groups.last_mut().expect("just pushed").push(node.clone());
            }
        }
        groups.sort_by(|a, b| a[0].text.to_lowercase().cmp(&b[0].text.to_lowercase()));
        let mut out = Vec::new();
        for g in groups {
            out.push(g[0].clone());
            if g.len() > 1 {
                out.extend(walk(&g[1..], depth + 1));
            }
        }
        out
    }
    let base = nodes.first().map(|n| n.depth).unwrap_or(0);
    walk(&nodes, base)
}

/// One node with its FAMILY already worked out — what a handler walks.
///
/// Every link is an INDEX: the same number the node event hands the handler in
/// `CONTROL-NODE-INDEX`, and the same number every lookup here takes. The index
/// **is** the node's handle. There is no node object to hold, which is
/// deliberate: a held object would go stale the moment `Items` changed, while
/// an index is simply re-read against whatever the tree holds now.
///
/// Links follow the order the nodes are WRITTEN, which is the order the indexes
/// are in. `Sorted` changes what the tree DRAWS; it never renumbers anybody's
/// handler, and it does not reorder anybody's traversal.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NodeInfo {
    /// This node's line in `Items` as written — its handle.
    pub index: usize,
    /// How deep it sits, 0 for a root.
    pub level: usize,
    pub text: String,
    /// The icon it names, when it names one.
    pub icon: Option<String>,
    /// Its own label colour and row colour, when it names them.
    pub color: Option<String>,
    pub background: Option<String>,
    /// The node it hangs under. `None` on a root.
    pub parent: Option<usize>,
    pub first_child: Option<usize>,
    pub last_child: Option<usize>,
    /// The next and previous node at THIS level under the same parent.
    pub next_sibling: Option<usize>,
    pub prev_sibling: Option<usize>,
    /// How many nodes hang directly under it — grandchildren not counted.
    pub child_count: usize,
    /// Root to node, joined by `/`. What tells two nodes with the same label
    /// apart, and what a handler can show a user as a location.
    pub path: String,
}

/// Every node in `Items`, in written order, with its family resolved.
///
/// Blank lines are dropped exactly as the renderer drops them, so a walk sees
/// precisely the nodes the tree draws.
pub fn nodes(items: &str) -> Vec<NodeInfo> {
    let parsed = parse(items);
    let depth_at = |p: usize| parsed[p].depth;
    parsed
        .iter()
        .enumerate()
        .map(|(p, node)| {
            let d = node.depth;
            // The parent is the nearest node ABOVE that is shallower. Walking
            // up rather than assuming `d - 1` tolerates a developer who
            // indented by four spaces in one place and two in another.
            let parent = (0..p).rev().find(|&q| depth_at(q) < d);
            // A sibling search stops at the first node shallower than this one
            // — that is where this node's parent's run ends.
            let prev_sibling = (0..p)
                .rev()
                .take_while(|&q| depth_at(q) >= d)
                .find(|&q| depth_at(q) == d);
            let next_sibling = (p + 1..parsed.len())
                .take_while(|&q| depth_at(q) >= d)
                .find(|&q| depth_at(q) == d);
            // Children are the run of deeper nodes immediately following. The
            // FIRST one sets the child level, so a subtree indented unevenly
            // still counts its own children rather than its grandchildren.
            let first_child = (p + 1 < parsed.len() && depth_at(p + 1) > d).then_some(p + 1);
            let (mut last_child, mut child_count) = (None, 0);
            if let Some(fc) = first_child {
                let child_level = depth_at(fc);
                for q in fc..parsed.len() {
                    if depth_at(q) <= d {
                        break;
                    }
                    if depth_at(q) == child_level {
                        last_child = Some(q);
                        child_count += 1;
                    }
                }
            }
            // The path is built from the ancestors, not from a running stack,
            // so it is correct for any node asked about on its own.
            let mut trail = vec![node.text.clone()];
            let mut up = parent;
            while let Some(q) = up {
                trail.push(parsed[q].text.clone());
                up = (0..q).rev().find(|&r| depth_at(r) < depth_at(q));
            }
            trail.reverse();
            NodeInfo {
                index: node.index,
                level: d,
                text: node.text.clone(),
                icon: node.icon.clone(),
                color: node.color.clone(),
                background: node.background.clone(),
                // Every link is reported as the node's WRITTEN index, not as
                // its position in this vector — the two differ the moment a
                // blank line is written, and only one of them is the handle an
                // event carries.
                parent: parent.map(|q| parsed[q].index),
                first_child: first_child.map(|q| parsed[q].index),
                last_child: last_child.map(|q| parsed[q].index),
                next_sibling: next_sibling.map(|q| parsed[q].index),
                prev_sibling: prev_sibling.map(|q| parsed[q].index),
                child_count,
                path: trail.join("/"),
            }
        })
        .collect()
}

/// The node with this index, or `None` when nothing is written there.
pub fn node_at(items: &str, index: usize) -> Option<NodeInfo> {
    nodes(items).into_iter().find(|n| n.index == index)
}

/// The index of the first node with this label — how a handler that knows only
/// a name (from `SelectedNode`, say) gets a handle to walk from.
pub fn index_of(items: &str, text: &str) -> Option<usize> {
    let needle = text.trim();
    nodes(items)
        .into_iter()
        .find(|n| n.text == needle)
        .map(|n| n.index)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A small tree with two roots, one of them a parent of two children, one
    /// of which is itself a parent — enough for every link to have both a
    /// present and an absent case.
    const TREE: &str = "Warehouse\n  Inbound\n    Dock A\n  Outbound\nOffice";

    #[test]
    fn a_node_knows_its_parent_and_its_children() {
        let n = nodes(TREE);
        assert_eq!(n[0].parent, None, "a root has no parent");
        assert_eq!(n[1].parent, Some(0), "Inbound hangs under Warehouse");
        assert_eq!(n[2].parent, Some(1), "Dock A hangs under Inbound");
        assert_eq!(n[0].first_child, Some(1));
        assert_eq!(n[0].last_child, Some(3), "Outbound is the last CHILD");
        assert_eq!(
            n[0].child_count, 2,
            "Warehouse has two children — Dock A is a grandchild"
        );
        assert_eq!(n[4].child_count, 0);
        assert_eq!(n[4].first_child, None);
    }

    #[test]
    fn siblings_link_within_their_own_parent() {
        let n = nodes(TREE);
        assert_eq!(n[1].next_sibling, Some(3), "Inbound → Outbound");
        assert_eq!(n[3].prev_sibling, Some(1), "and back again");
        assert_eq!(n[3].next_sibling, None, "Outbound is the last of its run");
        assert_eq!(
            n[2].next_sibling, None,
            "Dock A must not reach OUT of Inbound to find a sibling"
        );
        assert_eq!(n[0].next_sibling, Some(4), "the roots are siblings");
    }

    /// The index is the node's HANDLE, and a handle is a line number — so a
    /// blank line must not shift anybody. This is the whole reason links are
    /// reported as written indexes rather than as positions in the walk.
    #[test]
    fn a_blank_line_does_not_renumber_the_tree() {
        let n = nodes("Warehouse\n\n  Inbound\n\nOffice");
        assert_eq!(n.len(), 3, "blank lines are not nodes");
        assert_eq!(n[1].index, 2, "Inbound is written on line 2");
        assert_eq!(n[1].parent, Some(0));
        assert_eq!(n[2].index, 4);
        assert_eq!(
            n[0].first_child,
            Some(2),
            "the link carries the WRITTEN index, not the walk position"
        );
    }

    #[test]
    fn a_path_names_a_node_that_a_label_alone_cannot() {
        let n = nodes("North\n  Depot\nSouth\n  Depot");
        assert_eq!(n[1].path, "North/Depot");
        assert_eq!(n[3].path, "South/Depot");
        assert_eq!(n[0].path, "North", "a root is its own path");
    }

    /// A node carries its own icon, colour and background after TABs — the
    /// fields a handler reads back with `NodeIcon` / `NodeColor`.
    #[test]
    fn a_node_carries_its_own_fields_after_tabs() {
        let n = nodes("Plain\nFancy\tfolder\t#C81E1E\t#202020\nIconOnly\tdoc-text");
        assert_eq!(n[0].icon, None);
        assert_eq!(n[0].color, None);
        assert_eq!(n[1].icon.as_deref(), Some("folder"));
        assert_eq!(n[1].color.as_deref(), Some("#C81E1E"));
        assert_eq!(n[1].background.as_deref(), Some("#202020"));
        assert_eq!(n[2].icon.as_deref(), Some("doc-text"));
        assert_eq!(n[2].color, None, "an unwritten field is not a colour");

        // An EMPTY field is skipped over, so a colour can be named without an
        // icon — the reason each field is read independently.
        let skipped = nodes("Overdue\t\t#C81E1E");
        assert_eq!(skipped[0].icon, None);
        assert_eq!(skipped[0].color.as_deref(), Some("#C81E1E"));
    }

    #[test]
    fn a_name_finds_the_handle_to_walk_from() {
        assert_eq!(index_of(TREE, "Dock A"), Some(2));
        assert_eq!(index_of(TREE, "  Dock A  "), Some(2), "asked untrimmed");
        assert_eq!(index_of(TREE, "Nowhere"), None);
        assert_eq!(node_at(TREE, 2).map(|n| n.level), Some(2));
        assert_eq!(node_at(TREE, 99), None, "past the end is not a node");
    }

    /// Indentation is read per node rather than assumed to step by one, so a
    /// tree indented four-then-two still hangs together.
    #[test]
    fn uneven_indentation_still_finds_the_parent() {
        let n = nodes("Root\n    Child\n      Grandchild");
        assert_eq!(n[1].parent, Some(0));
        assert_eq!(n[2].parent, Some(1));
        assert_eq!(n[0].child_count, 1, "the grandchild is not a child");
    }
}
