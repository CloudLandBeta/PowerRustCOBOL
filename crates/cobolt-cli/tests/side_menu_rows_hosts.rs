// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 066 (R15; AC8): **the SideMenu's run-time rows are the same under `rcrun
//! run-form` and in a built binary** (`--ignored`: it compiles one).
//!
//! One main form with a SideMenu (two designed rows) and three buttons. The first
//! adds a section, a row and a row under a designed one; the second relabels,
//! badges and disables one of its own rows and tries to rename a designed row; the
//! third removes a row and clears the rest. After every click the test reads the
//! `RuntimeRows` property the host holds and the count the program wrote into a
//! label — the same readings on each host. The rows themselves are drawn by the
//! one shell the two hosts share, which `side_menu_hosted_controls` and the
//! `cobolt-forms` parity tests already pin; what this file adds is that the
//! program reaches them the same way from a built application.

use std::path::{Path, PathBuf};
use std::process::Command;

use cobolt_forms::menu::{save_menu, MenuDefinition, MenuItem};
use cobolt_forms::{Control, ControlType, EventBinding, Form, PropValue, Rect};
use serde_json::{json, Value};

struct Project {
    root: PathBuf,
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn rcrun() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_rcrun"))
}

fn handler(statements: &str) -> String {
    format!("       ENVIRONMENT DIVISION.\n       DATA DIVISION.\n       PROCEDURE DIVISION.\n{statements}\n")
}

fn button(form: &mut Form, id: &str, y: i32, statements: &str) {
    let mut c = Control::new(id, ControlType::Button, 320, y);
    c.rect = Rect::new(320, y, 180, 28);
    c.set_prop("Caption", PropValue::String(id.into()));
    c.events.push(EventBinding { event: "onClick".into(), paragraph: format!("{id}--ONCLICK"), code: handler(statements) });
    form.controls.push(c);
}

fn project(tag: &str) -> Project {
    let root = std::env::temp_dir().join(format!("prc-066-hosts-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    for d in ["forms", "generated", "src", "data"] {
        std::fs::create_dir_all(root.join(d)).unwrap();
    }
    let mut form = Form::new("ROWS-DEMO", "Run-time rows", 960, 540);
    form.main_form = true;
    let mut side = Control::new("SideMenu-1", ControlType::SideMenu, 0, 0);
    side.rect = Rect::new(0, 0, 240, 540);
    form.controls.push(side);
    form.sync_side_menu_footer_panels();
    form.sync_side_menu_header_panels();
    button(
        &mut form,
        "BTN-ADD",
        20,
        "           SideMenu-1::AddSection( \"Chats\" )\n\
         \x20          SideMenu-1::AddItem( \"chat-1\", \"First chat\", \"chat\" )\n\
         \x20          SideMenu-1::AddItem( \"chat-2\", \"Second chat\", \"chat\" )\n\
         \x20          SideMenu-1::AddItem( \"part-1\", \"A part\", \"\", \"docs\" )\n\
         \x20          MOVE SideMenu-1::GetCount() TO LBL-RESULT::Caption",
    );
    button(
        &mut form,
        "BTN-CHANGE",
        56,
        "           SideMenu-1::SetItemLabel( \"chat-1\", \"Renamed chat\" )\n\
         \x20          SideMenu-1::SetItemBadge( \"chat-1\", \"3\" )\n\
         \x20          SideMenu-1::SetItemEnabled( \"chat-2\", \"0\" )\n\
         \x20          SideMenu-1::SetItemLabel( \"home\", \"Nope\" )\n\
         \x20          MOVE SideMenu-1::GetCount() TO LBL-RESULT::Caption",
    );
    button(
        &mut form,
        "BTN-CLEAR",
        92,
        "           SideMenu-1::RemoveItem( \"chat-2\" )\n\
         \x20          SideMenu-1::RemoveItem( \"docs\" )\n\
         \x20          MOVE SideMenu-1::GetCount() TO LBL-RESULT::Caption\n\
         \x20          SideMenu-1::Clear()",
    );
    let mut result = Control::new("LBL-RESULT", ControlType::Label, 320, 140);
    result.rect = Rect::new(320, 140, 300, 24);
    result.set_prop("Caption", PropValue::String("-".into()));
    form.controls.push(result);
    cobolt_forms::save_form(&form, &root.join("forms/rows-demo.cfrm")).unwrap();

    let mut docs = MenuItem::new_action("docs", "Documents");
    docs.items.push(MenuItem::new_action("docs-new", "New"));
    let def = MenuDefinition { menu: vec![MenuItem::new_action("home", "Home"), docs], hash: String::new() };
    save_menu(&root.join("forms/SideMenu-1.menu.yaml"), &def).unwrap();

    let form = cobolt_forms::load_form(&root.join("forms/rows-demo.cfrm")).unwrap();
    std::fs::write(root.join("generated/rows-demo.cbl"), cobolt_codegen::generate(&form)).unwrap();
    std::fs::write(
        root.join("src/main.cbl"),
        "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. MAIN.\n       PROCEDURE DIVISION.\n           GOBACK.\n",
    )
    .unwrap();
    std::fs::write(
        root.join("rowshosts.project.toml"),
        "[project]\nname = \"RowsHosts\"\nversion = \"1.0.0\"\nmain = \"src/main.cbl\"\n\n\
         [files]\nsources = [\"src/main.cbl\"]\nforms = [\"forms/rows-demo.cfrm\"]\ngenerated = [\"generated/rows-demo.cbl\"]\n",
    )
    .unwrap();
    Project { root }
}

fn steps() -> Value {
    let click = |id: &str| json!({"event": {"control": id, "name": "onClick"}});
    let read = |ctrl: &str, prop: &str| json!({"read": {"control": ctrl, "property": prop}});
    let wait = || json!({"wait_ms": 500});
    Value::Array(vec![
        click("BTN-ADD"),
        wait(),
        read("SideMenu-1", "RuntimeRows"),
        read("LBL-RESULT", "Caption"),
        click("BTN-CHANGE"),
        wait(),
        read("SideMenu-1", "RuntimeRows"),
        read("LBL-RESULT", "Caption"),
        click("BTN-CLEAR"),
        wait(),
        read("LBL-RESULT", "Caption"),
        read("SideMenu-1", "RuntimeRows"),
    ])
}

/// Every `read` of a headless run, in order, as the host answered it.
fn readings(stdout: &str) -> Vec<String> {
    let report: Value = stdout
        .lines()
        .find_map(|l| l.strip_prefix("@RUN-FORM-RESULT "))
        .and_then(|j| serde_json::from_str(j).ok())
        .unwrap_or_else(|| panic!("no result line in:\n{stdout}"));
    report["steps"]
        .as_array()
        .map(|steps| steps.iter().filter_map(|st| st["value"].as_str().map(str::to_owned)).collect())
        .unwrap_or_default()
}

fn run_form(p: &Project) -> String {
    let script_path = p.root.join("steps.json");
    std::fs::write(&script_path, steps().to_string()).unwrap();
    let out = Command::new(rcrun())
        .args([
            "run-form",
            "forms/rows-demo.cfrm",
            "generated/rows-demo.cbl",
            "--designer",
            "--headless",
            script_path.to_str().unwrap(),
            "--headless-limit",
            "60",
        ])
        .current_dir(&p.root)
        .env("HOME", p.root.join("home"))
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(out.status.success(), "rcrun run-form failed:\n{stdout}\n{}", String::from_utf8_lossy(&out.stderr));
    stdout
}

fn summary(host: &str, got: &[String]) {
    println!("\n  066 AC8 — {host}");
    for (i, g) in got.iter().enumerate() {
        println!("    read {i:<2} {}", g.replace('\n', " | "));
    }
}

#[test]
fn run_time_rows_are_what_the_program_made_them_under_run_form() {
    let p = project("rf");
    let got = readings(&run_form(&p));
    summary("rcrun run-form", &got);
    check(&got);
}

/// The rows of a `RuntimeRows` reading as `(parent, id, label, badge, enabled, overlay)`.
fn rows(reading: &str) -> Vec<(Option<String>, String, String, Option<String>, bool, bool)> {
    let list: Vec<Value> = serde_json::from_str(reading).unwrap_or_else(|e| panic!("RuntimeRows is JSON: {e}\n{reading}"));
    list.iter()
        .map(|r| {
            let item = &r["item"];
            (
                r["parent"].as_str().map(str::to_owned),
                item["id"].as_str().unwrap().to_owned(),
                item["label"].as_str().unwrap().to_owned(),
                item["badge"].as_str().map(str::to_owned),
                item["enabled"].as_bool().unwrap(),
                r["overlay"].as_bool().unwrap_or(false),
            )
        })
        .collect()
}

/// What the program leaves, read after each click (six readings). `RuntimeRows` is the property the
/// host holds; the facts are asked of its parsed rows and not of its spelling.
fn check(got: &[String]) {
    assert_eq!(got.len(), 6, "six readings: {got:?}");
    // BTN-ADD: a section, two rows and a row under the designed `docs` — four of the program's own rows.
    let added = rows(&got[0]);
    let ids: Vec<&str> = added.iter().map(|r| r.1.as_str()).collect();
    assert_eq!(ids, ["section-1", "chat-1", "chat-2", "part-1"], "{added:?}");
    assert_eq!(added[3].0.as_deref(), Some("docs"), "the nested row sits under the designed one");
    assert_eq!(got[1], "4", "GetCount() counts the program's own rows");
    // BTN-CHANGE: relabel and badge one of its rows, disable another; the designed `home` is only
    // overlaid (a label the host draws), never moved or counted.
    let changed = rows(&got[2]);
    let chat1 = changed.iter().find(|r| r.1 == "chat-1").unwrap();
    assert_eq!((chat1.2.as_str(), chat1.3.as_deref(), chat1.4), ("Renamed chat", Some("3"), true));
    assert!(!changed.iter().find(|r| r.1 == "chat-2").unwrap().4, "chat-2 is disabled");
    let home = changed.iter().find(|r| r.1 == "home").unwrap();
    assert!(home.5 && home.2 == "Nope", "a designed row is only overlaid: {home:?}");
    assert_eq!(got[3], "4", "an overlay is not one of the program's rows");
    // BTN-CLEAR: chat-2 removed, the designed `docs` refused (3 left), then everything of the program's gone.
    assert_eq!(got[4], "3", "chat-2 went, the designed row stayed");
    let left = rows(&got[5]);
    assert!(left.iter().all(|r| r.5), "only overlays are left after Clear(): {left:?}");
    assert_eq!(left.len(), 1);
}

/// The built binary reads the same. Builds one: `--ignored`.
#[test]
#[ignore = "builds a real binary; run with --ignored"]
fn run_time_rows_are_the_same_in_a_built_binary() {
    let p = project("bin");
    let direct = readings(&run_form(&p));
    let ws = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap();
    let opts = cobolt_compiler::BuildOptions { verbose: false, workspace_root: Some(ws), debug: true, ..Default::default() };
    let built = cobolt_compiler::build_project(&p.root.join("rowshosts.project.toml"), &opts)
        .unwrap_or_else(|e| panic!("the build failed: {e}"));
    let script = p.root.join("bin-steps.json");
    std::fs::write(&script, steps().to_string()).unwrap();
    let out = Command::new(&built.binary_path)
        .env("PRC_HEADLESS_SCRIPT", &script)
        .env("PRC_HEADLESS_LIMIT", "60")
        .env("HOME", p.root.join("home"))
        .current_dir(&p.root)
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(out.status.success(), "{stdout}\n{}", String::from_utf8_lossy(&out.stderr));
    let got = readings(&stdout);
    summary("rcrun run-form", &direct);
    summary("the built binary", &got);
    check(&got);
    assert_eq!(got, direct, "the built binary reads what Run Form reads");
}
