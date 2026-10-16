// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 082 — an agent edits a menu's designed items with `set_menu_items`.
//!
//! The fixture is a copy of PowerDemo3's `sidebar-form` with its two menu files and the twelve
//! *Responsive Layout* forms, so the tree is the real one (94 items) and `open-form:` targets have
//! real form formats to be judged against. Each test applies a change-set to a real
//! `DesignerPanel` and reads the `.menu.yaml` back from disk.

use super::*;
use crate::agent::{parse_change_set, AgentChangeSet};
use cobolt_forms::menu::{load_menu, menu_yaml_path, MenuItem};
use std::path::{Path, PathBuf};

const MENU: &str = "SideMenu-1";
/// *Responsive Layout*, the group the operator asked to sort.
const GROUP: &str = "ocwm";

fn cs(json: &serde_json::Value) -> AgentChangeSet {
    parse_change_set(&format!("```json\n{json}\n```")).expect("the change-set parses")
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap().flatten() {
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// A designer on a private copy of PowerDemo3's sidebar form.
struct Rig {
    d: DesignerPanel,
    dir: PathBuf,
}

impl Rig {
    fn new(tag: &str) -> Self {
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/PowerDemo3/forms");
        let dir = std::env::temp_dir().join(format!("prc-082-{tag}-{}", std::process::id())).join("forms");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for name in ["sidebar-form.cfrm", "SideMenu-1.menu.yaml", "MenuBar-1.menu.yaml"] {
            std::fs::copy(src.join(name), dir.join(name)).unwrap();
        }
        copy_dir(&src.join("Responsive Layout"), &dir.join("Responsive Layout"));
        let form = cobolt_forms::load_form(&dir.join("sidebar-form.cfrm")).expect("the form loads");
        let mut d = DesignerPanel::new(form);
        d.cfrm_dir = Some(dir.clone());
        Rig { d, dir }
    }

    fn path(&self) -> PathBuf {
        menu_yaml_path(&self.dir, MENU)
    }

    fn file(&self) -> Vec<u8> {
        std::fs::read(self.path()).unwrap()
    }

    fn menu(&self) -> Vec<MenuItem> {
        load_menu(&self.path()).expect("the menu loads, hash included").menu
    }

    /// The tree as an agent sends it: every item with its id and label, groups with their children.
    fn tree(&self) -> serde_json::Value {
        fn json(i: &MenuItem) -> serde_json::Value {
            let mut v = serde_json::json!({ "id": i.id, "label": i.label });
            if matches!(i.item_type, cobolt_forms::menu::MenuItemType::Separator) {
                v["type"] = "separator".into();
            }
            if !i.items.is_empty() {
                v["items"] = i.items.iter().map(json).collect();
            }
            v
        }
        self.menu().iter().map(json).collect()
    }

    fn set_items(&self, tree: serde_json::Value) -> AgentChangeSet {
        cs(&serde_json::json!({ "operations": [ { "op": "set_menu_items", "control_id": MENU, "items": tree } ] }))
    }

    /// Verdicts of the form-free checks only; the Designer's own refusals show in `last_change_outcome`.
    fn verdicts(&self, set: &AgentChangeSet) -> Vec<Option<String>> {
        crate::agent::validate_planned(set, &self.d.form).0
    }
}

impl Drop for Rig {
    fn drop(&mut self) {
        if let Some(parent) = self.dir.parent() {
            let _ = std::fs::remove_dir_all(parent);
        }
    }
}

fn find_json<'a>(items: &'a mut [serde_json::Value], id: &str) -> Option<&'a mut serde_json::Value> {
    for item in items.iter_mut() {
        if item["id"] == id {
            return Some(item);
        }
        if let Some(found) = item.get_mut("items").and_then(|c| c.as_array_mut()).and_then(|c| find_json(c, id)) {
            return Some(found);
        }
    }
    None
}

/// The children of the *Responsive Layout* group inside the tree an agent sends, wherever it sits.
fn group_mut(tree: &mut serde_json::Value) -> &mut Vec<serde_json::Value> {
    find_json(tree.as_array_mut().unwrap(), GROUP)
        .and_then(|g| g["items"].as_array_mut())
        .expect("the Responsive Layout group")
}

/// The first line at which two trees, printed one field per line, stop agreeing.
fn first_difference(a: &[MenuItem], b: &[MenuItem]) -> String {
    let (a, b) = (format!("{a:#?}"), format!("{b:#?}"));
    let at = a.lines().zip(b.lines()).position(|(x, y)| x != y).unwrap_or(0);
    let context = |text: &str| text.lines().skip(at.saturating_sub(12)).take(16).collect::<Vec<_>>().join("\n");
    format!("--- actual\n{}\n--- expected\n{}", context(&a), context(&b))
}

fn labels(items: &[MenuItem]) -> Vec<String> {
    items.iter().map(|i| i.label.clone()).collect()
}

fn ids_of(items: &[MenuItem], out: &mut Vec<String>) {
    for i in items {
        out.push(i.id.clone());
        ids_of(&i.items, out);
    }
}

fn find_item<'a>(items: &'a [MenuItem], id: &str) -> Option<&'a MenuItem> {
    items.iter().find_map(|i| if i.id == id { Some(i) } else { find_item(&i.items, id) })
}

fn find_item_mut<'a>(items: &'a mut [MenuItem], id: &str) -> Option<&'a mut MenuItem> {
    for i in items.iter_mut() {
        if i.id == id {
            return Some(i);
        }
        if let Some(found) = find_item_mut(&mut i.items, id) {
            return Some(found);
        }
    }
    None
}

fn group_of(menu: &[MenuItem]) -> &MenuItem {
    find_item(menu, GROUP).expect("the Responsive Layout group")
}

/// AC1, AC2 — the operator's own request: *Samples > Responsive Layout* A–Z. The agent sends only the
/// id and the label of each item, and everything else the file holds (icons, actions, enabled flags,
/// the other groups) is exactly what it was.
#[test]
fn sorting_the_responsive_layout_group_changes_only_that_group_and_keeps_every_id() {
    let mut rig = Rig::new("sort");
    let before = rig.menu();
    let total = {
        let mut all = Vec::new();
        ids_of(&before, &mut all);
        all.len()
    };
    let original = labels(&group_of(&before).items);
    let mut sorted = original.clone();
    sorted.sort_by_key(|l| l.to_lowercase());
    assert_ne!(original, sorted, "the fixture's group is not already sorted, or the test proves nothing");

    let mut tree = rig.tree();
    group_mut(&mut tree).sort_by_key(|i| i["label"].as_str().unwrap().to_lowercase());
    let set = rig.set_items(tree);
    assert!(rig.verdicts(&set).iter().all(Option::is_none), "{:?}", rig.verdicts(&set));
    assert_eq!(rig.d.apply_agent_change_set(&set), 1, "one command");

    let after = rig.menu();
    assert_eq!(labels(&group_of(&after).items), sorted, "A to Z");
    // The expected file: the old tree with that one group reordered, nothing else.
    let mut expected = before.clone();
    find_item_mut(&mut expected, GROUP).unwrap().items.sort_by_key(|i| i.label.to_lowercase());
    assert!(after == expected, "icons, actions and every other item are untouched; first difference:\n{}", first_difference(&after, &expected));
    let mut ids_before = Vec::new();
    let mut ids_after = Vec::new();
    ids_of(&before, &mut ids_before);
    ids_of(&after, &mut ids_after);
    ids_before.sort();
    ids_after.sort();
    assert_eq!(ids_before, ids_after, "every item kept its id");
    println!("spec 082 sort: {} items in the group, {total} in the menu, group A-Z, nothing else changed", sorted.len());
}

/// AC3 — a new item gets an id the menu editor would give: four lowercase letters, unique in the
/// window. An id that is not in that format, and an id another menu of the window already has, are
/// replaced the same way.
#[test]
fn a_new_item_gets_a_fresh_id_unique_in_the_window() {
    let mut rig = Rig::new("fresh");
    // Another menu on the window: its ids are taken.
    rig.d.form.controls.push(Control::new("MenuBar-1", ControlType::MenuBar, 0, 0));
    let other = load_menu(&menu_yaml_path(&rig.dir, "MenuBar-1")).unwrap().menu;
    let other_id = other[0].id.clone();
    let before_ids = {
        let mut all = Vec::new();
        ids_of(&rig.menu(), &mut all);
        all
    };

    let mut tree = rig.tree();
    group_mut(&mut tree).push(serde_json::json!({ "label": "Responsive: zeta", "action": "open-form:responsive-grid-form" }));
    group_mut(&mut tree).push(serde_json::json!({ "id": "NOT-AN-ID", "label": "Responsive: eta" }));
    group_mut(&mut tree).push(serde_json::json!({ "id": other_id, "label": "Responsive: theta" }));
    let set = rig.set_items(tree);
    rig.d.apply_agent_change_set(&set);

    let after = rig.menu();
    let group = group_of(&after);
    let added: Vec<&MenuItem> = group.items.iter().rev().take(3).collect();
    assert_eq!(added.iter().map(|i| i.label.as_str()).collect::<Vec<_>>(), ["Responsive: theta", "Responsive: eta", "Responsive: zeta"]);
    for item in &added {
        assert!(crate::panels::item_ids::is_generated(&item.id), "four lowercase letters: {}", item.id);
        assert!(!before_ids.contains(&item.id), "{} was already in the menu", item.id);
        assert_ne!(item.id, other_id, "an id of another menu of the window is taken");
    }
    let mut all = Vec::new();
    ids_of(&after, &mut all);
    let mut unique = all.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(all.len(), unique.len(), "no id is used twice");
    assert_eq!(added[2].action.as_deref(), Some("open-form:responsive-grid-form"));
}

/// AC4 — what the menu editor would not allow is refused with a reason that names the item, and the
/// file is not touched.
#[test]
fn a_tree_the_menu_editor_would_refuse_is_refused_and_the_file_is_untouched() {
    let mut rig = Rig::new("refuse");
    let original = rig.file();

    // Four levels deep.
    let deep = serde_json::json!([{ "label": "L1", "items": [{ "label": "L2", "items": [{ "label": "L3", "items": [{ "label": "L4-too-deep" }] }] }] }]);
    let set = rig.set_items(deep);
    let why = rig.verdicts(&set)[0].clone().expect("refused");
    assert!(why.contains("L4-too-deep") && why.contains("levels"), "{why}");

    // An id used twice.
    let dupe = serde_json::json!([{ "id": "abcd", "label": "First" }, { "id": "abcd", "label": "Second" }]);
    let why = rig.verdicts(&rig.set_items(dupe))[0].clone().expect("refused");
    assert!(why.contains("abcd") && why.contains("First") && why.contains("Second"), "{why}");

    // An action that does not exist, a target with no form, an action a MenuBar does not offer.
    let bad = |action: &str| rig.verdicts(&rig.set_items(serde_json::json!([{ "label": "Item", "action": action }])))[0].clone();
    assert!(bad("explode").unwrap().contains("not an action"));
    assert!(bad("open-form:").unwrap().contains("needs the form"));
    assert!(bad("home:x").unwrap().contains("takes no target"));
    assert!(bad("event").is_none() && bad("home").is_none() && bad("close-application").is_none());

    // Nothing to replace, a control that is not there, and a control that is not a menu.
    assert!(rig.verdicts(&rig.set_items(serde_json::json!([])))[0].as_deref().unwrap().contains("empty"));
    let elsewhere = |control: &str| {
        let set = cs(&serde_json::json!({ "operations": [ { "op": "set_menu_items", "control_id": control, "items": [{ "label": "x" }] } ] }));
        rig.verdicts(&set)[0].clone()
    };
    assert!(elsewhere("NOPE").unwrap().contains("No control named"));
    assert!(elsewhere("Panel-1").unwrap().contains("only a SideMenu or a MenuBar"));

    // An open-form: that points at a Standalone form (the sidebar form itself is one).
    let embed_standalone = rig.set_items(serde_json::json!([{ "label": "Back to start", "action": "open-form:sidebar-form" }, { "label": "Fine", "action": "open-form:responsive-grid-form" }]));
    assert!(rig.verdicts(&embed_standalone)[0].is_none(), "the form-free check cannot know the format");
    assert_eq!(rig.d.apply_agent_change_set(&embed_standalone), 0, "nothing is applied");
    let ledger = rig.d.last_change_outcome.clone();
    assert!(ledger.contains("NOT applied") && ledger.contains("Back to start") && ledger.contains("Standalone"), "{ledger}");
    assert!(ledger.contains("open-standalone-sync:sidebar-form"), "the remedy is named: {ledger}");

    // A MenuBar is not offered `home`.
    rig.d.form.controls.push(Control::new("MenuBar-9", ControlType::MenuBar, 0, 0));
    let bar = cs(&serde_json::json!({ "operations": [ { "op": "set_menu_items", "control_id": "MenuBar-9", "items": [{ "label": "Home", "action": "home" }] } ] }));
    assert!(rig.verdicts(&bar)[0].as_deref().unwrap().contains("SideMenu"), "{:?}", rig.verdicts(&bar));

    assert_eq!(rig.file(), original, "the file was never touched");
    assert!(rig.d.undo_stack.is_empty(), "and no command was recorded");
}

/// A refused operation does not stop the others: the valid one in the same change-set applies.
#[test]
fn a_refused_menu_operation_does_not_stop_the_next_one() {
    let mut rig = Rig::new("partial");
    let set = cs(&serde_json::json!({ "operations": [
        { "op": "set_menu_items", "control_id": MENU, "items": [{ "label": "Back", "action": "open-form:sidebar-form" }] },
        { "op": "set_menu_items", "control_id": MENU, "items": [{ "id": "zzzz", "label": "Only item" }] },
    ] }));
    rig.d.apply_agent_change_set(&set);
    let menu = rig.menu();
    assert_eq!(labels(&menu), ["Only item"], "the second operation applied alone");
    let ledger = rig.d.last_change_outcome.clone();
    assert!(ledger.contains("- NOT applied: set_menu_items") && ledger.contains("- applied: set_menu_items"), "{ledger}");
}

/// AC5, R6 — dropping an item lists it, and names the handler that still mentions its id. The
/// handler is not changed.
#[test]
fn dropping_an_item_lists_it_and_the_handlers_that_name_its_id_and_changes_no_code() {
    let mut rig = Rig::new("drop");
    let handler = "           EVALUATE SelectedItemId\n               WHEN \"rspa\"\n                   CONTINUE\n           END-EVALUATE".to_string();
    rig.d.form.controls.iter_mut().find(|c| c.id == "Panel-1").unwrap().events.push(cobolt_forms::model::EventBinding {
        event: "onClick".into(),
        paragraph: "PANEL-1--ONCLICK".into(),
        code: handler.clone(),
    });
    let mut tree = rig.tree();
    group_mut(&mut tree).retain(|i| i["id"] != "rspa" && i["id"] != "rspb");
    let set = rig.set_items(tree);
    rig.d.apply_agent_change_set(&set);

    let ledger = rig.d.last_change_outcome.clone();
    assert!(ledger.contains("'Responsive: anchors and limits' (id rspa) is no longer in the menu"), "{ledger}");
    assert!(ledger.contains("Panel-1::onClick"), "the handler that names rspa is listed: {ledger}");
    assert!(ledger.contains("'Responsive: docking' (id rspb) is no longer in the menu."), "an id nobody names is listed alone: {ledger}");
    assert!(!ledger.contains("rspb) is no longer in the menu; code"), "{ledger}");
    let kept = &rig.d.form.find_control("Panel-1").unwrap().events[0].code;
    assert_eq!(kept, &handler, "the code is never changed");
    assert_eq!(group_of(&rig.menu()).items.len(), 10, "and the two items are gone from the file");
}

/// AC6, R7 — one Undo puts the file back byte for byte; Redo applies it again. Two operations on the
/// same menu in one change-set are still one step, and still undo to the original file.
#[test]
fn one_undo_restores_the_file_byte_for_byte_even_after_two_operations_on_one_menu() {
    let mut rig = Rig::new("undo");
    let original = rig.file();

    let mut tree = rig.tree();
    group_mut(&mut tree).reverse();
    let first = rig.set_items(tree);
    rig.d.apply_agent_change_set(&first);
    let changed = rig.file();
    assert_ne!(changed, original);
    assert_eq!(rig.d.undo_stack.len(), 1);
    rig.d.undo();
    assert_eq!(rig.file(), original, "one Undo gives the file back byte for byte");
    rig.d.redo();
    assert_eq!(rig.file(), changed, "and Redo does it again");
    rig.d.undo();

    let mut a = rig.tree();
    group_mut(&mut a).reverse();
    let mut b = rig.tree();
    group_mut(&mut b).truncate(3);
    let set = cs(&serde_json::json!({ "operations": [
        { "op": "set_menu_items", "control_id": MENU, "items": a },
        { "op": "set_menu_items", "control_id": MENU, "items": b },
    ] }));
    let depth = rig.d.undo_stack.len();
    rig.d.apply_agent_change_set(&set);
    assert_eq!(rig.d.undo_stack.len(), depth + 1, "two operations, one step");
    assert_eq!(group_of(&rig.menu()).items.len(), 3, "the later operation won");
    rig.d.undo();
    assert_eq!(rig.file(), original, "and one Undo goes back past both");
}

/// AC7, R10 — the context the next request is built from lists the new order.
#[test]
fn the_context_after_the_change_shows_the_new_order() {
    let mut rig = Rig::new("context");
    let mut tree = rig.tree();
    group_mut(&mut tree).sort_by_key(|i| i["label"].as_str().unwrap().to_lowercase());
    let set = rig.set_items(tree);
    rig.d.apply_agent_change_set(&set);

    let ctx = crate::agent::menu_items_context(&rig.d.form, &rig.dir);
    let at = |label: &str| ctx.find(&format!("\"{label}\"")).unwrap_or_else(|| panic!("{label} is listed"));
    assert!(
        at("Responsive: anchors and limits") < at("Responsive: breakpoints") && at("Responsive: breakpoints") < at("Responsive: collision limits"),
        "the new order is the one shown"
    );
    assert!(ctx.contains("set_menu_items"), "and the block says how to change it");
}

/// R3 — a field the agent does not send keeps its value; an empty string clears the optional ones;
/// what the tool never offers (a hosted control) is kept.
#[test]
fn a_field_left_out_keeps_its_value_and_an_empty_string_clears_it() {
    use crate::agent::{plan_menu_items, MenuItemKind, MenuItemSpec};
    let def = cobolt_forms::menu::MenuDefinition {
        menu: vec![{
            let mut a = MenuItem::new_action("abcd", "Search");
            a.icon = Some("search".into());
            a.badge = Some("New".into());
            a.accelerator = Some("Ctrl+F".into());
            a.action = Some("event".into());
            a.control = Some("TextBox-1".into());
            a.preserve_previous_form = true;
            a
        }],
        hash: String::new(),
    };
    let spec = |s: MenuItemSpec| plan_menu_items(Some(&def), &Default::default(), &[s]).unwrap().def.menu.remove(0);

    let kept = spec(MenuItemSpec { id: Some("abcd".into()), label: "Find".into(), ..Default::default() });
    assert_eq!(kept.label, "Find");
    assert_eq!(kept.icon.as_deref(), Some("search"));
    assert_eq!(kept.badge.as_deref(), Some("New"));
    assert_eq!(kept.accelerator.as_deref(), Some("Ctrl+F"));
    assert_eq!(kept.control.as_deref(), Some("TextBox-1"), "the hosted control is kept");
    assert!(kept.preserve_previous_form);

    let cleared = spec(MenuItemSpec {
        id: Some("abcd".into()),
        label: "Find".into(),
        icon: Some(String::new()),
        badge: Some(String::new()),
        accelerator: Some(" ".into()),
        enabled: Some(false),
        preserve_previous_form: Some(false),
        ..Default::default()
    });
    assert_eq!((cleared.icon, cleared.badge, cleared.accelerator), (None, None, None));
    assert!(!cleared.enabled && !cleared.preserve_previous_form);
    assert_eq!(cleared.control.as_deref(), Some("TextBox-1"));

    let divider = spec(MenuItemSpec { item_type: MenuItemKind::Separator, ..Default::default() });
    assert!(matches!(divider.item_type, cobolt_forms::menu::MenuItemType::Separator));
    assert_ne!(divider.id, "abcd", "an item with no id is a new item");
}

/// A MenuBar is edited the same way, and a menu that has no file yet is created.
#[test]
fn a_menu_with_no_file_yet_is_created_and_undo_removes_it() {
    let mut rig = Rig::new("create");
    rig.d.form.controls.push(Control::new("MenuBar-9", ControlType::MenuBar, 0, 0));
    let path = menu_yaml_path(&rig.dir, "MenuBar-9");
    assert!(!path.exists());
    let set = cs(&serde_json::json!({ "operations": [ { "op": "set_menu_items", "control_id": "MenuBar-9", "items": [
        { "label": "File", "items": [{ "label": "Open", "action": "event", "accelerator": "Ctrl+O" }, { "type": "separator" }, { "label": "Quit", "action": "close-application" }] }
    ] } ] }));
    rig.d.apply_agent_change_set(&set);
    let menu = load_menu(&path).expect("written through the menu editor's own save").menu;
    assert_eq!(menu[0].items.len(), 3);
    assert_eq!(menu[0].items[0].accelerator.as_deref(), Some("Ctrl+O"));
    rig.d.undo();
    assert!(!path.exists(), "a menu that did not exist is removed by Undo");
}

/// The contract, the prompts and the Guide know the operation: a correct change-set is not rejected as
/// unknown, and the JSON an agent writes parses into the operation.
#[test]
fn the_json_the_contract_teaches_parses() {
    let set = cs(&serde_json::json!({ "operations": [ { "op": "set_menu_items", "control_id": "SIDEMENU-1", "items": [
        { "id": "abcd", "label": "Samples", "icon": "folder", "items": [{ "id": "efgh", "label": "Charts", "action": "open-form:charts-form", "enabled": true, "badge": "6", "badge_style": "count" }] },
        { "label": "Quit", "action": "close-application" }
    ] } ] }));
    assert_eq!(set.operations.len(), 1);
    let crate::agent::AgentOp::SetMenuItems { control_id, items } = &set.operations[0] else { panic!("{:?}", set.operations[0]) };
    assert_eq!(control_id, "SIDEMENU-1");
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].items[0].badge_style.as_deref(), Some("count"));
    assert!(crate::agent::op_headline(&set.operations[0]).contains("set_menu_items SIDEMENU-1"));
    let contract = crate::tool_exec::CHANGE_SET_CONTRACT;
    assert!(contract.contains("set_menu_items") && contract.contains("REPLACES"));
    let specialist = cobolt_agents::specialist::Specialist::builtin("FormsDesigner").expect("a built-in specialist");
    assert!(cobolt_agents::specialist::compose_system_prompt("", Some(&specialist)).contains("set_menu_items"));
    assert!(crate::llm::DEFAULT_FORM_DESIGNER_AGENT_PROMPT.contains("set_menu_items"));
    assert!(crate::llm::DEFAULT_PEDANTIC_UI_PROMPT.contains("set_menu_items"));
}

/// Grace's own summary of a plan names the menu, instead of letting the operation fall through as nothing.
#[test]
fn grace_summarises_a_menu_operation() {
    let ops: Vec<serde_json::Value> = serde_json::from_str(
        r#"[{"op":"set_menu_items","control_id":"SideMenu-1","items":[{"label":"A"},{"label":"B","items":[{"label":"C"}]}]}]"#,
    )
    .unwrap();
    assert_eq!(crate::grace_host::summarize_operations(&ops), ["Set the items of menu SideMenu-1 (2 at the top level)."]);
}
