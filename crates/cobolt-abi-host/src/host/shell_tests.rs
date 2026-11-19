// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The phone shell's verbs (WASM-012 T15, ABI A26) as the host sees them: refused with `-6`
//! outside the shell, a command to the shell and a wait inside it, and the answer read back as
//! the application's `SecureStatus` and `SecureValue`.

use super::*;
use cobolt_wasm_abi::manifest::{Limits, VariantInfo};

fn host(shell: bool) -> Host {
    let manifest = Manifest {
        contract: None,
        remote: Vec::new(),
        capabilities: Vec::new(),
        abi: 1,
        generator: "test".into(),
        project: "demo".into(),
        variant: VariantInfo { multi_memory: true, conformance: false },
        limits: Limits::default(),
        names: vec![],
        events: vec![],
        forms: vec![],
        builtins: vec!["SECURESTORE".into(), "SECURELOAD".into(), "SECUREDELETE".into()],
        imports: vec![],
        sizes: Default::default(),
    };
    let mut h = Host::new(manifest, HostOptions::default());
    h.shell = shell;
    h
}

fn shell_calls(h: &mut Host) -> Vec<(u32, &'static str, Vec<(&'static str, String)>)> {
    h.effects.drain(..).filter_map(|e| if let Effect::Shell { id, command, args } = e { Some((id, command, args)) } else { None }).collect()
}

#[test]
fn outside_the_shell_each_secure_verb_answers_status_minus_6_and_says_why() {
    let mut h = host(false);
    h.opts.debug_level = 1;
    h.app.insert("securevalue".into(), "stale".into());
    for verb in ["SecureStore", "SecureLoad", "SecureDelete"] {
        assert!(matches!(h.verb(1, 2, verb, &["session", "token"]), Ok(None)), "{verb}: the handler goes on, as after a FILE STATUS");
        assert_eq!((h.app_get("SecureStatus"), h.app_get("SecureValue")), ("-6".to_owned(), String::new()), "{verb}");
        let said = h.effects.iter().any(|e| matches!(e, Effect::Log { text, .. } if text.contains(&format!("COBOL::{verb}")) && text.contains("mobile shell")));
        assert!(said, "{verb}: the console says why");
    }
    assert!(shell_calls(&mut h).is_empty(), "nothing is asked of a shell that is not there");
    assert!(h.inflight.is_none() || h.inflight.as_ref().is_some_and(|f| f.wait.is_none()), "no task waits");
    println!("outside the shell: 3 verbs answered SecureStatus -6 with SecureValue empty and a console line, no command sent, no wait");
}

#[test]
fn inside_the_shell_a_verb_becomes_a_command_and_its_answer_is_read_back() {
    let mut h = host(true);
    h.verb(1, 2, "SecureStore", &["  session  ", "token-123      "]).unwrap();
    let calls = shell_calls(&mut h);
    assert_eq!(calls.len(), 1);
    let (id, command, args) = calls[0].clone();
    assert_eq!((command, args), ("secure_store", vec![("key", "session".to_owned()), ("value", "token-123".to_owned())]), "the key trimmed, the value without its padding");
    h.shell_answer(id, 0, "");
    assert_eq!(h.app_get("SecureStatus"), "0");

    h.verb(1, 2, "SecureLoad", &["session"]).unwrap();
    let (id, command, args) = shell_calls(&mut h).remove(0);
    assert_eq!((command, args), ("secure_load", vec![("key", "session".to_owned())]));
    assert_eq!(h.app_get("SecureValue"), "", "cleared while the shell is asked");
    h.shell_answer(id, 0, "token-123");
    assert_eq!((h.app_get("SecureStatus"), h.app_get("SecureValue")), ("0".to_owned(), "token-123".to_owned()));

    h.verb(1, 2, "SecureDelete", &["absent"]).unwrap();
    let (id, command, _) = shell_calls(&mut h).remove(0);
    assert_eq!(command, "secure_delete");
    h.shell_answer(id, 1, "");
    assert_eq!(h.app_get("SecureStatus"), "1", "nothing to delete");

    h.verb(1, 2, "SecureLoad", &["k"]).unwrap();
    let (id, _, _) = shell_calls(&mut h).remove(0);
    h.shell_answer(id, -6, "");
    assert_eq!(h.app_get("SecureStatus"), "-6", "a refusal of the shell");
    println!("inside the shell: 4 commands (store, load, delete, load refused), statuses 0 0 1 -6, the loaded value read back");
}

/// `SideMenu::ActivateItem(id)` (spec 049) answers what the interpreter answers — `1` for a row of the menu, designed or the
/// program's own, `0` for an empty or unknown id — and leaves the click to the page, which holds the menu and the ContentPane.
/// It used to answer nothing: the browser had no such method, so a shell could not open its first screen from `onLoad`.
#[test]
fn activate_item_answers_from_the_menu_and_leaves_the_click_to_the_page() {
    use cobolt_forms::menu::{MenuDefinition, MenuItem};
    let mut h = host(false);
    let (tree, side) = crate::tree::testing::tree_with("SideMenu", "SIDEMENU-1", &[]);
    let form = tree.form_name.clone();
    h.trees.insert(1, tree);
    let mut off = MenuItem::new_action("off", "Off");
    off.enabled = false;
    let dash = MenuItem { action: Some("open-form:dashboard-form".into()), ..MenuItem::new_action("dash", "Dashboard") };
    h.menus.insert(&form, "SIDEMENU-1", MenuDefinition { menu: vec![dash, off], hash: String::new() });
    let answer = |h: &mut Host, id: &str| h.control_method(1, side, "ActivateItem", &[id]).unwrap().value.unwrap_or_default();
    let commands = |h: &mut Host| -> Vec<Vec<String>> {
        h.trees.get_mut(&1).unwrap().take_changes().into_iter().filter_map(|c| match c {
            Change::Command { name, args, .. } if name == "activate-item" => Some(args),
            _ => None,
        }).collect()
    };
    assert_eq!(answer(&mut h, " dash "), "1");
    assert_eq!(commands(&mut h), vec![vec!["dash".to_owned()]], "one click for the page, the id trimmed");
    assert_eq!(answer(&mut h, "off"), "1", "a disabled row is in the menu: the interpreter answers 1 and the click does nothing");
    assert_eq!(commands(&mut h).len(), 1);
    for id in ["", "nope", "DASH"] {
        assert_eq!(answer(&mut h, id), "0", "`{id}`: not a row of the menu, as `has_item` reads it");
        assert!(commands(&mut h).is_empty(), "`{id}`: nothing for the page");
    }
    let (other, lst) = crate::tree::testing::tree_with("ListBox", "LST-1", &[]);
    h.trees.insert(2, other);
    assert!(h.control_method(2, lst, "ActivateItem", &["dash"]).is_err(), "only a SideMenu has the method");
    println!("ActivateItem on the web: 2 rows answered 1 with a click for the page, 3 ids answered 0 with none, a ListBox refused");
}

/// The SideMenu's run-time rows (spec 066) are the interpreter's on every surface: the same dispatch
/// (`cobolt_forms::menu::runtime::row_method`), the same `RuntimeRows` written, the same answers. Before
/// DESKTOP-WASM-001 T31 the host took `AddItem` for a list's and counted lines, so a program's rows
/// never reached the menu. `Collapse` and `Open` write the rail's `Collapsed`, as the interpreter does;
/// on the desktop `ActivateItem` writes the request the desktop's shell watches instead of a click
/// for the page.
#[test]
fn side_menu_rows_are_the_interpreters_on_every_surface() {
    use cobolt_forms::menu::runtime::{parse_rows, ACTIVATE_ITEM_PROP, RUNTIME_ROWS_PROP};
    use cobolt_forms::menu::{MenuDefinition, MenuItem};
    let mut summary = Vec::new();
    for desktop in [false, true] {
        let mut h = host(false);
        let (mut tree, side) = crate::tree::testing::tree_with("SideMenu", "SIDEMENU-1", &[]);
        tree.desktop = desktop;
        let form = tree.form_name.clone();
        h.trees.insert(1, tree);
        let mut docs = MenuItem::new_action("docs", "Documents");
        docs.items.push(MenuItem::new_action("docs-new", "New"));
        h.menus.insert(&form, "SIDEMENU-1", MenuDefinition { menu: vec![MenuItem::new_action("home", "Home"), docs], hash: String::new() });
        let call = |h: &mut Host, m: &str, args: &[&str]| h.control_method(1, side, m, args).unwrap().value.unwrap_or_default();
        let answers = vec![
            call(&mut h, "AddSection", &["Chats"]),
            call(&mut h, "AddItem", &["chat-1", "First chat", "chat"]),
            call(&mut h, "AddItem", &["part-1", "A part", "", "docs"]),
            call(&mut h, "SetItemLabel", &["home", "Nope"]),
            call(&mut h, "RemoveItem", &["docs"]),
            call(&mut h, "GetCount", &[]),
            call(&mut h, "HasItem", &["part-1"]),
        ];
        assert_eq!(answers, ["section-1", "1", "1", "1", "0", "3", "1"], "desktop {desktop}: AddSection's id, three changes, a designed row refused, 3 own rows");
        let rows = parse_rows(&h.trees[&1].get(side, RUNTIME_ROWS_PROP));
        let ids: Vec<&str> = rows.iter().map(|r| r.item.id.as_str()).collect();
        assert_eq!(ids, ["section-1", "chat-1", "part-1", "home"], "desktop {desktop}: the rows, then the overlay on the designed `home`");
        assert!(rows[3].overlay && rows[3].item.label == "Nope");
        assert_eq!(call(&mut h, "Clear", &[]), "1");
        assert_eq!(call(&mut h, "GetCount", &[]), "0", "Clear() leaves only the overlays");
        assert_eq!(call(&mut h, "Collapse", &[]), "1");
        let canonical = |v: &str| cobolt_forms::model::property_runtime_text("SideMenu", "Collapsed", v);
        assert_eq!(h.trees[&1].get(side, "Collapsed"), canonical("1"));
        assert_eq!(call(&mut h, "Open", &[]), "1");
        assert_eq!(h.trees[&1].get(side, "Collapsed"), canonical("0"));
        let _ = h.trees.get_mut(&1).unwrap().take_changes();
        assert_eq!(call(&mut h, "ActivateItem", &["docs-new"]), "1");
        assert_eq!(call(&mut h, "ActivateItem", &["docs-new"]), "1");
        let clicks = h.trees.get_mut(&1).unwrap().take_changes().into_iter().filter(|c| matches!(c, Change::Command { name, .. } if name == "activate-item")).count();
        let token = h.trees[&1].get(side, ACTIVATE_ITEM_PROP);
        if desktop {
            assert_eq!((clicks, token.as_str()), (0, "docs-new#2"), "the desktop's shell watches a new request each time");
        } else {
            assert_eq!((clicks, token.as_str()), (2, ""), "the page is sent a click each time");
        }
        summary.push(format!("{}: 7 row calls answered as the interpreter, 4 rows, Clear, Collapse/Open, ActivateItem ×2 ({})", if desktop { "desktop" } else { "web" }, if desktop { "request docs-new#2" } else { "2 clicks" }));
    }
    for line in &summary {
        println!("{line}");
    }
    println!("result: 2 of 2 surfaces passed");
}

/// The SideMenu row methods (spec 066) in the browser answer and leave what the interpreter answers and leaves. They fell
/// to the list verbs: `AddItem` appended to `Items`, which the menu never draws, and answered nothing (a `PIC 9` item read
/// 0), `GetCount()` counted those `Items`, and `SetItemEnabled` was refused with run-time error -4 (the Cnd_PRC report).
#[test]
fn side_menu_row_methods_edit_the_rows_the_menu_draws() {
    use cobolt_forms::menu::runtime as rt;
    use cobolt_forms::menu::{MenuDefinition, MenuItem};
    let mut h = host(false);
    let (tree, side) = crate::tree::testing::tree_with("SideMenu", "SIDEMENU-1", &[]);
    let form = tree.form_name.clone();
    h.trees.insert(1, tree);
    h.menus.insert(&form, "SIDEMENU-1", MenuDefinition { menu: vec![MenuItem::new_action("home", "Home")], hash: String::new() });
    let call = |h: &mut Host, m: &str, args: &[&str]| h.control_method(1, side, m, args).unwrap().value.unwrap_or_default();
    let drawn = |h: &Host| -> Vec<(String, bool)> {
        let designed = h.menus.get(&form, "SIDEMENU-1").map(|d| d.menu.clone()).unwrap_or_default();
        let rows = rt::parse_rows(&h.trees[&1].get(side, rt::RUNTIME_ROWS_PROP));
        rt::merge_rows(&designed, &rows).iter().map(|i| (i.id.clone(), i.enabled)).collect()
    };
    let mut answers = Vec::new();
    for k in 1..=134 {
        answers.push(call(&mut h, "AddItem", &[&format!(" m{k} "), &format!("Option {k}"), "", "", "open-form:f"]));
    }
    assert!(answers.iter().all(|a| a == "1"), "each AddItem answers 1");
    assert_eq!(call(&mut h, "GetCount", &[]), "134", "the program's own rows");
    assert_eq!(drawn(&h).len(), 135, "the designed row and the 134 added, as the menu draws them");
    assert_eq!(drawn(&h)[1], ("m1".to_owned(), true), "the id trimmed");
    assert!(h.trees[&1].get(side, "Items").is_empty(), "nothing goes to the list property the menu does not draw");
    assert_eq!(call(&mut h, "SetItemEnabled", &["home", "0"]), "1", "a designed row");
    assert_eq!(call(&mut h, "SetItemEnabled", &["m2", "0"]), "1", "a program's row");
    assert_eq!(call(&mut h, "SetItemEnabled", &["nope", "1"]), "0", "no such row");
    assert_eq!(call(&mut h, "AddItem", &["home", "Twin"]), "0", "a designed id is refused");
    let d = drawn(&h);
    assert_eq!((d[0].1, d[2].1, d[3].1), (false, false, true));
    assert_eq!(call(&mut h, "HasItem", &["m134"]), "1");
    assert_eq!(call(&mut h, "Clear", &[]), "1");
    assert_eq!(call(&mut h, "GetCount", &[]), "0");
    assert_eq!(drawn(&h), vec![("home".to_owned(), false)], "Clear keeps what the program set on a designed row");
    println!("SideMenu rows on the web: 134 AddItem answered 1 and drawn, GetCount 134, SetItemEnabled on a designed and a program row, 2 refusals answered 0, Clear");
}
