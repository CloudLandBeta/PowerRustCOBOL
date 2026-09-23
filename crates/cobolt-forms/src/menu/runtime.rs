// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Rows a running program adds to a SideMenu (spec 066).
//!
//! A SideMenu's designed rows live in its `.menu.yaml` and belong to the
//! developer. A program adds its own at run time — a list of conversations, the
//! documents in a folder — and they travel as one run-time-only property,
//! [`RUNTIME_ROWS_PROP`]: a JSON list of [`RuntimeRow`]s in the order they were
//! added. Every surface that draws the menu merges them after the designed rows
//! with [`merge_rows`], so the rail in a window and the rail in the shell lay
//! out the same list.
//!
//! The operations here are pure — `(designed menu, rows) → rows or a refusal` —
//! so the interpreter can answer a program synchronously ("did that work?")
//! and they can be tested without one. A program may never remove, rename or
//! shadow a designed row: those calls are refused, and the menu is unchanged.

use super::{MenuItem, MenuItemType, MAX_DEPTH};
use serde::{Deserialize, Serialize};

/// The run-time-only SideMenu property that carries [`RuntimeRow`]s as JSON.
pub const RUNTIME_ROWS_PROP: &str = "RuntimeRows";

/// The action a row gets when the program names none: it raises
/// `onMenuItemClick`. Named rather than left empty so an iconed row shows on
/// the collapsed rail exactly as a designed one does.
pub const DEFAULT_ACTION: &str = "event";

/// One row a program added. `item.items` is always empty: children point at
/// their parent through [`RuntimeRow::parent`] instead, so a row can hang
/// under a designed row as easily as under another run-time one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuntimeRow {
    #[serde(default)]
    pub parent: Option<String>,
    pub item: MenuItem,
}

/// Why an operation left the menu as it was.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// The id names a row from the designed menu, which a program may not
    /// change, remove or replace.
    DesignedRow,
    /// No row has this id.
    NoSuchRow,
    /// The parent named does not exist, or is a section title.
    NoSuchParent,
    /// The row would sit deeper than [`MAX_DEPTH`].
    TooDeep,
    /// A row needs an id.
    EmptyId,
    /// A row cannot hang under itself or under one of its own children.
    Cycle,
}

/// The rows in a [`RUNTIME_ROWS_PROP`] value. Empty or unreadable is no rows —
/// a menu never fails to draw because of what a program wrote.
pub fn parse_rows(json: &str) -> Vec<RuntimeRow> {
    if json.trim().is_empty() {
        return Vec::new();
    }
    serde_json::from_str(json).unwrap_or_default()
}

/// The [`RUNTIME_ROWS_PROP`] value for `rows`; empty when there are none.
pub fn rows_json(rows: &[RuntimeRow]) -> String {
    if rows.is_empty() {
        String::new()
    } else {
        serde_json::to_string(rows).unwrap_or_default()
    }
}

/// The designed rows followed by the run-time ones. A row with a parent is
/// that parent's last child; a row whose parent is gone is dropped.
pub fn merge_rows(designed: &[MenuItem], rows: &[RuntimeRow]) -> Vec<MenuItem> {
    let mut merged = designed.to_vec();
    for row in rows {
        let mut item = row.item.clone();
        item.items.clear();
        match &row.parent {
            None => merged.push(item),
            Some(parent) => {
                if let Some(p) = find_mut(&mut merged, parent) {
                    p.items.push(item);
                }
            }
        }
    }
    merged
}

fn find_mut<'a>(items: &'a mut [MenuItem], id: &str) -> Option<&'a mut MenuItem> {
    for item in items {
        if item.id == id {
            return Some(item);
        }
        if let Some(found) = find_mut(&mut item.items, id) {
            return Some(found);
        }
    }
    None
}

fn find<'a>(items: &'a [MenuItem], id: &str) -> Option<&'a MenuItem> {
    items.iter().find_map(|item| {
        if item.id == id {
            Some(item)
        } else {
            find(&item.items, id)
        }
    })
}

/// 1-based depth of `id` in `items`: a top-level row is 1.
fn depth_of(items: &[MenuItem], id: &str) -> Option<usize> {
    for item in items {
        if item.id == id {
            return Some(1);
        }
        if let Some(d) = depth_of(&item.items, id) {
            return Some(d + 1);
        }
    }
    None
}

/// How many levels a row and everything under it occupy: 1 for a leaf.
fn height(item: &MenuItem) -> usize {
    1 + item.items.iter().map(height).max().unwrap_or(0)
}

/// Does `ancestor` sit on `id`'s parent chain (or is it `id` itself)?
fn is_self_or_ancestor(rows: &[RuntimeRow], ancestor: &str, id: &str) -> bool {
    let mut cursor = Some(id.to_owned());
    let mut guard = 0;
    while let Some(current) = cursor {
        if current == ancestor {
            return true;
        }
        guard += 1;
        if guard > rows.len() + 1 {
            return false;
        }
        cursor = rows
            .iter()
            .find(|r| r.item.id == current)
            .and_then(|r| r.parent.clone());
    }
    false
}

fn guard_designed(designed: &[MenuItem], id: &str) -> Result<(), Refusal> {
    if find(designed, id).is_some() {
        Err(Refusal::DesignedRow)
    } else {
        Ok(())
    }
}

fn row_mut<'a>(rows: &'a mut [RuntimeRow], id: &str) -> Option<&'a mut RuntimeRow> {
    rows.iter_mut().find(|r| r.item.id == id)
}

/// Add a row — or, when a run-time row already has this id, replace it where
/// it stands (its children stay with it). `action` empty ⇒ [`DEFAULT_ACTION`].
pub fn add_item(
    designed: &[MenuItem],
    rows: &mut Vec<RuntimeRow>,
    id: &str,
    label: &str,
    icon: &str,
    parent: &str,
    action: &str,
) -> Result<(), Refusal> {
    let id = id.trim();
    if id.is_empty() {
        return Err(Refusal::EmptyId);
    }
    guard_designed(designed, id)?;
    let parent = Some(parent.trim()).filter(|p| !p.is_empty()).map(str::to_owned);

    let merged = merge_rows(designed, rows);
    // The levels this row brings with it: itself, plus any children it
    // already has if this is a replacement.
    let brings = find(&merged, id).map_or(1, height);
    let depth = match &parent {
        None => 1,
        Some(p) => {
            if is_self_or_ancestor(rows, id, p) {
                return Err(Refusal::Cycle);
            }
            let host = find(&merged, p).ok_or(Refusal::NoSuchParent)?;
            if host.item_type == MenuItemType::Separator {
                return Err(Refusal::NoSuchParent);
            }
            depth_of(&merged, p).unwrap_or(1) + 1
        }
    };
    if depth + brings - 1 > MAX_DEPTH {
        return Err(Refusal::TooDeep);
    }

    let mut item = MenuItem::new_action(id, label.trim());
    item.icon = Some(icon.trim().to_owned()).filter(|s| !s.is_empty());
    item.action = Some(action.trim())
        .filter(|s| !s.is_empty())
        .unwrap_or(DEFAULT_ACTION)
        .to_owned()
        .into();
    match row_mut(rows, id) {
        Some(existing) => {
            // Keep what the program set with the other setters.
            item.badge = existing.item.badge.clone();
            item.badge_style = existing.item.badge_style;
            item.enabled = existing.item.enabled;
            existing.item = item;
            existing.parent = parent;
        }
        None => rows.push(RuntimeRow { parent, item }),
    }
    Ok(())
}

/// Add a section title. Sections get their own ids (`section-1`, …) so
/// `RemoveItem` can take one out again; the id is returned.
pub fn add_section(designed: &[MenuItem], rows: &mut Vec<RuntimeRow>, title: &str) -> String {
    let merged = merge_rows(designed, rows);
    let id = (1..)
        .map(|n| format!("section-{n}"))
        .find(|id| find(&merged, id).is_none())
        .expect("an unbounded range always yields a free id");
    let mut item = MenuItem::new_separator(id.clone());
    item.label = title.trim().to_owned();
    rows.push(RuntimeRow { parent: None, item });
    id
}

/// Change one field of a run-time row.
pub fn set_field(
    designed: &[MenuItem],
    rows: &mut [RuntimeRow],
    id: &str,
    set: impl FnOnce(&mut MenuItem),
) -> Result<(), Refusal> {
    let id = id.trim();
    guard_designed(designed, id)?;
    let row = row_mut(rows, id).ok_or(Refusal::NoSuchRow)?;
    set(&mut row.item);
    Ok(())
}

/// Remove a run-time row and every row under it.
pub fn remove_item(
    designed: &[MenuItem],
    rows: &mut Vec<RuntimeRow>,
    id: &str,
) -> Result<(), Refusal> {
    let id = id.trim();
    guard_designed(designed, id)?;
    if !rows.iter().any(|r| r.item.id == id) {
        return Err(Refusal::NoSuchRow);
    }
    let snapshot = rows.clone();
    rows.retain(|r| !is_self_or_ancestor(&snapshot, id, &r.item.id));
    Ok(())
}

/// Is there a row — designed or run-time — with this id?
pub fn has_item(designed: &[MenuItem], rows: &[RuntimeRow], id: &str) -> bool {
    find(&merge_rows(designed, rows), id.trim()).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn designed() -> Vec<MenuItem> {
        let mut docs = MenuItem::new_action("docs", "Documents");
        docs.items.push(MenuItem::new_action("docs-new", "New"));
        vec![MenuItem::new_action("home", "Home"), docs]
    }

    fn ids(items: &[MenuItem]) -> Vec<String> {
        let mut out = Vec::new();
        for i in items {
            out.push(i.id.clone());
            for c in &i.items {
                out.push(format!("{}/{}", i.id, c.id));
                for g in &c.items {
                    out.push(format!("{}/{}/{}", i.id, c.id, g.id));
                }
            }
        }
        out
    }

    /// Every operation, with what it must do — printed as a table so a run
    /// shows which cases were exercised, not only that they passed.
    #[test]
    fn the_operations_do_what_they_say_and_refuse_what_they_must() {
        let d = designed();
        let mut rows = Vec::new();
        let mut table: Vec<(&str, String)> = Vec::new();
        let mut check = |case: &'static str, got: Result<(), Refusal>, want: Result<(), Refusal>| {
            assert_eq!(got, want, "{case}");
            table.push((case, format!("{got:?}")));
        };

        check("add top-level row", add_item(&d, &mut rows, "c1", "Chat 1", "", "", ""), Ok(()));
        check("add row under a designed row", add_item(&d, &mut rows, "d1", "Doc", "", "docs", ""), Ok(()));
        check("add row under a run-time row", add_item(&d, &mut rows, "c1a", "Part", "", "c1", ""), Ok(()));
        check("third level is allowed", add_item(&d, &mut rows, "c1a1", "Deep", "", "c1a", ""), Ok(()));
        check("fourth level is refused", add_item(&d, &mut rows, "c1a1x", "Too", "", "c1a1", ""), Err(Refusal::TooDeep));
        check("a designed id cannot be added", add_item(&d, &mut rows, "home", "X", "", "", ""), Err(Refusal::DesignedRow));
        check("an unknown parent is refused", add_item(&d, &mut rows, "z", "Z", "", "nowhere", ""), Err(Refusal::NoSuchParent));
        check("an empty id is refused", add_item(&d, &mut rows, " ", "Z", "", "", ""), Err(Refusal::EmptyId));
        check("a row cannot hang under its own child", add_item(&d, &mut rows, "c1", "Chat 1", "", "c1a", ""), Err(Refusal::Cycle));
        check("a designed row cannot be renamed", set_field(&d, &mut rows, "home", |i| i.label = "X".into()), Err(Refusal::DesignedRow));
        check("a designed row cannot be removed", remove_item(&d, &mut rows, "docs"), Err(Refusal::DesignedRow));
        check("relabel a run-time row", set_field(&d, &mut rows, "c1", |i| i.label = "Renamed".into()), Ok(()));
        check("an unknown row cannot be changed", set_field(&d, &mut rows, "nope", |i| i.enabled = false), Err(Refusal::NoSuchRow));

        let merged = merge_rows(&d, &rows);
        assert_eq!(
            ids(&merged),
            ["home", "docs", "docs/docs-new", "docs/d1", "c1", "c1/c1a", "c1/c1a/c1a1"],
            "designed first, run-time after, children last under their parent"
        );
        assert_eq!(merged[2].label, "Renamed");
        assert_eq!(merged[2].action.as_deref(), Some(DEFAULT_ACTION));

        // Replacing keeps the row where it was.
        add_item(&d, &mut rows, "c2", "Chat 2", "", "", "").unwrap();
        add_item(&d, &mut rows, "c1", "Chat One", "chat", "", "open-form:CHAT").unwrap();
        let merged = merge_rows(&d, &rows);
        assert_eq!(merged[2].id, "c1", "a replaced row keeps its position");
        assert_eq!(merged[2].action.as_deref(), Some("open-form:CHAT"));

        check("remove takes the children with it", remove_item(&d, &mut rows, "c1"), Ok(()));
        assert_eq!(ids(&merge_rows(&d, &rows)), ["home", "docs", "docs/docs-new", "docs/d1", "c2"]);

        let sec = add_section(&d, &mut rows, "History");
        assert_eq!(sec, "section-1");
        check("a section is not a parent", add_item(&d, &mut rows, "y", "Y", "", &sec, ""), Err(Refusal::NoSuchParent));
        assert!(has_item(&d, &rows, "home") && has_item(&d, &rows, "c2") && !has_item(&d, &rows, "c1a"));

        // The value round-trips through the property.
        assert_eq!(parse_rows(&rows_json(&rows)), rows);
        assert!(parse_rows("").is_empty() && parse_rows("not json").is_empty());
        assert_eq!(rows_json(&[]), "");

        println!("\n  RuntimeRows — {} cases", table.len());
        for (case, got) in &table {
            println!("    {case:<42} → {got}");
        }
    }
}
