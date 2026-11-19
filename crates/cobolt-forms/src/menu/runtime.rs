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
//! and they can be tested without one. A program may never remove or shadow a
//! designed row, nor change its id, icon, badge or action: those calls are
//! refused, and the menu is unchanged.
//!
//! Two things about a designed row ARE the program's to change: its **label**
//! (so a menu designed in the RAD can follow the interface language) and
//! whether it is **enabled** (so a menu can wait until the application is set
//! up). They travel as an *overlay* row — a [`RuntimeRow`] with
//! [`RuntimeRow::overlay`] set and the designed row's id — which [`merge_rows`]
//! applies to that row in place instead of adding one.

use super::{MenuItem, MenuItemType, MAX_DEPTH};
use serde::{Deserialize, Serialize};

/// The run-time-only SideMenu property that carries [`RuntimeRow`]s as JSON.
pub const RUNTIME_ROWS_PROP: &str = "RuntimeRows";

/// `ActivateItem(id)`'s request: `<id>#<sequence>`, written on the SideMenu and
/// watched by the shell, which performs that row's action as a click would. A
/// new sequence number is a new request, so activating the same row twice
/// acts twice.
pub const ACTIVATE_ITEM_PROP: &str = "ActivateItemRequest";

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
    /// This row adjusts the DESIGNED row with the same id — its label and its
    /// enabled flag — rather than being a row of its own.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub overlay: bool,
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
    for row in rows.iter().filter(|r| r.overlay) {
        if let Some(d) = find_mut(&mut merged, &row.item.id) {
            d.label = row.item.label.clone();
            d.enabled = row.item.enabled;
        }
    }
    for row in rows.iter().filter(|r| !r.overlay) {
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
        None => rows.push(RuntimeRow { parent, item, overlay: false }),
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
    rows.push(RuntimeRow { parent: None, item, overlay: false });
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
    let row = rows
        .iter_mut()
        .find(|r| !r.overlay && r.item.id == id)
        .ok_or(Refusal::NoSuchRow)?;
    set(&mut row.item);
    Ok(())
}

/// Change a row's label — a designed row's too, which is how a menu designed
/// in the RAD follows the interface language.
pub fn set_label(
    designed: &[MenuItem],
    rows: &mut Vec<RuntimeRow>,
    id: &str,
    label: &str,
) -> Result<(), Refusal> {
    let label = label.to_owned();
    set_presentation(designed, rows, id, move |i| i.label = label)
}

/// Enable or disable a row — a designed row's too, so a menu can stay shut
/// until the application is set up.
pub fn set_enabled(
    designed: &[MenuItem],
    rows: &mut Vec<RuntimeRow>,
    id: &str,
    enabled: bool,
) -> Result<(), Refusal> {
    set_presentation(designed, rows, id, move |i| i.enabled = enabled)
}

/// A run-time row is changed in place; a designed one gets (or updates) its
/// overlay, which starts as a copy of the designed row so the field not being
/// set keeps its designed value.
fn set_presentation(
    designed: &[MenuItem],
    rows: &mut Vec<RuntimeRow>,
    id: &str,
    set: impl FnOnce(&mut MenuItem),
) -> Result<(), Refusal> {
    let id = id.trim();
    let Some(original) = find(designed, id) else {
        return set_field(designed, rows, id, set);
    };
    let at = match rows.iter().position(|r| r.overlay && r.item.id == id) {
        Some(at) => at,
        None => {
            let mut item = original.clone();
            item.items.clear();
            rows.push(RuntimeRow { parent: None, item, overlay: true });
            rows.len() - 1
        }
    };
    set(&mut rows[at].item);
    Ok(())
}

/// The rows the program added, without the overlays on designed rows.
pub fn own_row_count(rows: &[RuntimeRow]) -> usize {
    rows.iter().filter(|r| !r.overlay).count()
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

/// What a SideMenu row method answered ([`row_method`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RowAnswer {
    /// What the program receives: `1` for a change and `0` for a refusal (the
    /// rows are then as they were), the count for `GetCount`, `1`/`0` for
    /// `HasItem`, the new section's id for `AddSection`.
    pub value: String,
    /// The rows changed: the host writes them back to [`RUNTIME_ROWS_PROP`].
    pub changed: bool,
}

/// The SideMenu methods that read or edit the program's rows, answered from
/// the designed menu and the rows alone, so every host — the interpreter and
/// the browser player — gives a program the same answer and leaves the same
/// rows. `method` is upper case; `arg(i)` is the i-th argument, trimmed.
/// `None` when `method` is not one of them: `ActivateItem`, `Collapse`,
/// `Open` and the rest are the host's.
pub fn row_method(
    designed: &[MenuItem],
    rows: &mut Vec<RuntimeRow>,
    method: &str,
    arg: &dyn Fn(usize) -> String,
) -> Option<RowAnswer> {
    let truthy = |s: &str| matches!(s.trim().to_ascii_uppercase().as_str(), "1" | "TRUE" | "YES" | "ON");
    let outcome: Result<(), Refusal> = match method {
        // AddItem(id, label [, icon [, parent-id [, action]]])
        "ADDITEM" => add_item(designed, rows, &arg(0), &arg(1), &arg(2), &arg(3), &arg(4)),
        "ADDSECTION" => {
            let id = add_section(designed, rows, &arg(0));
            return Some(RowAnswer { value: id, changed: true });
        }
        "REMOVEITEM" => remove_item(designed, rows, &arg(0)),
        // A designed row's label and enabled flag are the program's too
        // (a translated interface; a menu that waits for set-up).
        "SETITEMLABEL" => set_label(designed, rows, &arg(0), &arg(1)),
        "SETITEMICON" => {
            let v = arg(1);
            set_field(designed, rows, &arg(0), |i| i.icon = Some(v).filter(|s| !s.is_empty()))
        }
        "SETITEMBADGE" => {
            let v = arg(1);
            set_field(designed, rows, &arg(0), |i| i.badge = Some(v).filter(|s| !s.is_empty()))
        }
        "SETITEMENABLED" => set_enabled(designed, rows, &arg(0), truthy(&arg(1))),
        // Spec 066 Phase C — the control a run-time row hosts in place of its
        // label (a designed row's is the designer's, so it is refused like the
        // other fields). Empty gives the control back.
        "SETITEMCONTROL" => {
            let v = arg(1).trim().to_owned();
            set_field(designed, rows, &arg(0), |i| i.control = Some(v).filter(|s| !s.is_empty()))
        }
        "SETITEMACTION" => {
            let v = arg(1);
            set_field(designed, rows, &arg(0), |i| {
                i.action = Some(if v.is_empty() { DEFAULT_ACTION.to_owned() } else { v })
            })
        }
        // Only the program's rows: the designed menu is the developer's, and
        // so is what the program set on it (its labels, its state).
        "CLEAR" => {
            rows.retain(|r| r.overlay);
            Ok(())
        }
        "GETCOUNT" => return Some(RowAnswer { value: own_row_count(rows).to_string(), changed: false }),
        "HASITEM" => {
            let v = if has_item(designed, rows, &arg(0)) { "1" } else { "0" };
            return Some(RowAnswer { value: v.into(), changed: false });
        }
        _ => return None,
    };
    Some(match outcome {
        Ok(()) => RowAnswer { value: "1".into(), changed: true },
        Err(_) => RowAnswer { value: "0".into(), changed: false },
    })
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
        check("a designed row's icon cannot be changed", set_field(&d, &mut rows, "home", |i| i.icon = Some("X".into())), Err(Refusal::DesignedRow));
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

    /// A menu designed in the RAD shows in the designer and the preview, and
    /// the running program still translates its labels and holds rows shut
    /// until the application is set up. Nothing else about a designed row
    /// moves: its place, id, icon and action are the developer's.
    #[test]
    fn a_designed_row_takes_a_label_and_an_enabled_flag_and_nothing_else() {
        let d = designed();
        let mut rows = Vec::new();
        add_item(&d, &mut rows, "c1", "Chat 1", "", "", "").unwrap();

        set_label(&d, &mut rows, "home", "Início").unwrap();
        set_enabled(&d, &mut rows, "docs-new", false).unwrap();
        set_enabled(&d, &mut rows, "home", false).unwrap();
        set_label(&d, &mut rows, "c1", "Conversa 1").unwrap();
        assert_eq!(set_label(&d, &mut rows, "nope", "X"), Err(Refusal::NoSuchRow));
        assert_eq!(
            set_field(&d, &mut rows, "home", |i| i.action = Some("x".into())),
            Err(Refusal::DesignedRow),
            "a designed row's action stays the developer's"
        );

        let merged = merge_rows(&d, &rows);
        assert_eq!(ids(&merged), ["home", "docs", "docs/docs-new", "c1"], "an overlay adds no row");
        assert_eq!((merged[0].label.as_str(), merged[0].enabled), ("Início", false), "both fields land, the second keeps the first");
        assert_eq!(merged[0].icon, d[0].icon);
        assert!(!merged[1].items[0].enabled, "a nested designed row can be shut too");
        assert_eq!(merged[1].items[0].label, "New", "its label keeps the designed value");
        assert_eq!(merged[2].label, "Conversa 1");
        assert_eq!(own_row_count(&rows), 1, "the overlays are not rows of the program's");

        // A round trip through the property, and an old value with no overlay
        // field at all, both read back.
        let back = parse_rows(&rows_json(&rows));
        assert_eq!(back, rows);
        let old = parse_rows(r#"[{"parent":null,"item":{"id":"c9","label":"Old","type":"action"}}]"#);
        assert_eq!(old.len(), 1, "a value written before overlays existed still reads");
        assert!(!old[0].overlay);
    }

    /// The answers a program receives, from the one function every host calls:
    /// `1` for a change (the rows to write back), `0` for a refusal (the rows
    /// as they were), the count of the program's own rows, `None` for a method
    /// that is the host's.
    #[test]
    fn row_method_answers_what_every_host_answers() {
        let d = designed();
        let mut rows = Vec::new();
        let call = |rows: &mut Vec<RuntimeRow>, m: &str, args: &[&str]| {
            let owned: Vec<String> = args.iter().map(|s| s.to_string()).collect();
            row_method(&d, rows, m, &|i| owned.get(i).cloned().unwrap_or_default())
        };
        let answer = |value: &str, changed: bool| Some(RowAnswer { value: value.into(), changed });
        let mut table = Vec::new();
        for (m, args, want) in [
            ("ADDITEM", vec!["c1", "Chat 1"], answer("1", true)),
            ("ADDITEM", vec!["c2", "Chat 2", "", "c1"], answer("1", true)),
            ("ADDITEM", vec!["home", "Twin"], answer("0", false)),
            ("SETITEMENABLED", vec!["home", "0"], answer("1", true)),
            ("SETITEMENABLED", vec!["c1", "FALSE"], answer("1", true)),
            ("SETITEMENABLED", vec!["nope", "1"], answer("0", false)),
            ("GETCOUNT", vec![], answer("2", false)),
            ("HASITEM", vec!["c2"], answer("1", false)),
            ("HASITEM", vec!["nope"], answer("0", false)),
            ("REMOVEITEM", vec!["home"], answer("0", false)),
            ("ACTIVATEITEM", vec!["home"], None),
            ("COLLAPSE", vec![], None),
        ] {
            let before = rows.clone();
            let got = call(&mut rows, m, &args);
            assert_eq!(got, want, "{m}({args:?})");
            if !got.as_ref().is_some_and(|a| a.changed) {
                assert_eq!(rows, before, "{m}({args:?}): a refusal or a read leaves the rows alone");
            }
            table.push(format!("{m:<15} {:<22} -> {:?}", format!("{args:?}"), got.map(|a| a.value)));
        }
        let merged = merge_rows(&d, &rows);
        assert_eq!(ids(&merged), ["home", "docs", "docs/docs-new", "c1", "c1/c2"]);
        assert!(!merged[0].enabled && !merged[2].enabled, "SetItemEnabled reaches a designed row and a program's row");
        assert_eq!(call(&mut rows, "CLEAR", &[]), answer("1", true));
        assert_eq!(own_row_count(&rows), 0, "Clear takes the program's rows");
        assert!(!merge_rows(&d, &rows)[0].enabled, "and leaves what it set on a designed row");
        println!("{}", table.join("\n"));
    }
}
