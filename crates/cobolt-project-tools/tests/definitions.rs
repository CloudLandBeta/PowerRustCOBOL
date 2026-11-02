// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **Definition for AI over MCP**: `describe`, `set_definition` and the Check warning, end to end on a
//! copy of the check fixture.

use std::path::{Path, PathBuf};

use cobolt_project_tools::{HeadlessHost, ProjectHost, ProjectTools};
use serde_json::{json, Value};

const FIXTURE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/check_project");

fn fixture(name: &str) -> PathBuf {
    let dst = std::env::temp_dir().join(format!("prc-defs-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dst);
    copy_dir(Path::new(FIXTURE), &dst);
    dst
}

fn copy_dir(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).unwrap();
    for e in std::fs::read_dir(src).unwrap().flatten() {
        let to = dst.join(e.file_name());
        if e.path().is_dir() {
            copy_dir(&e.path(), &to);
        } else {
            std::fs::copy(e.path(), to).unwrap();
        }
    }
}

fn call(tools: &mut ProjectTools<impl ProjectHost>, name: &str, args: Value) -> Result<Value, String> {
    tools.call(name, &args)
}

fn warning_about_definitions(check: &Value) -> Vec<String> {
    check["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["severity"] == "warning")
        .filter_map(|d| d["message"].as_str())
        .filter(|m| m.contains("Definition for AI"))
        .map(str::to_owned)
        .collect()
}

#[test]
fn an_agent_reads_writes_and_is_reminded_of_definitions() {
    let dir = fixture("flow");
    let mut tools = ProjectTools::new(HeadlessHost::new(&dir, "test"));
    let form = "forms/MAIN-FORM.cfrm";

    // The overview: the form, and how much of it says nothing about itself.
    let overview = call(&mut tools, "describe", json!({})).unwrap();
    let f = &overview["forms"][0];
    assert_eq!(f["path"], form);
    assert_eq!(f["kind"], "desktop");
    assert!(f["without_definition"].as_u64().unwrap() >= 2, "the form and its button: {overview:#}");
    assert_eq!(overview["min_definition_chars"], 20);

    // One form in full: every control with whether it is visual, its events and its definition.
    let full = call(&mut tools, "describe", json!({ "path": form })).unwrap();
    let btn = full["controls"].as_array().unwrap().iter().find(|c| c["id"] == "BTN-OK").expect("the button");
    assert_eq!(btn["type"], "Button");
    assert_eq!(btn["visual"], true);
    assert_eq!(btn["definition"], "");
    assert!(btn["events"].as_array().unwrap().iter().any(|e| e == "onClick"));

    // Check says it, once for the file.
    let before = call(&mut tools, "check", json!({ "path": form })).unwrap();
    let warned = warning_about_definitions(&before);
    assert_eq!(warned.len(), 1, "one warning per file, not per control: {before:#}");
    assert!(warned[0].contains("BTN-OK") && warned[0].contains("form MAIN-FORM"), "{}", warned[0]);

    // Write them: the form's and the button's, with lines.
    let good = "Main screen of the customer desk.\nOpened when the application starts.";
    let r = call(&mut tools, "set_definition", json!({ "path": form, "element": "form", "text": good })).unwrap();
    assert_eq!(r["thin"], false);
    let r = call(
        &mut tools,
        "set_definition",
        json!({ "path": form, "element": "control", "id": "BTN-OK", "text": "Confirms the customer data and saves the record when every field is valid." }),
    )
    .unwrap();
    assert_eq!(r["thin"], false);

    let after = call(&mut tools, "describe", json!({ "path": form })).unwrap();
    assert_eq!(after["form"]["definition"].as_str().unwrap().replace("\r\n", "\n"), good);
    assert_eq!(after["controls"].as_array().unwrap().iter().find(|c| c["id"] == "BTN-OK").unwrap()["definition"], "Confirms the customer data and saves the record when every field is valid.");
    assert_eq!(after["without_definition"], 0, "{after:#}");
    assert!(warning_about_definitions(&call(&mut tools, "check", json!({ "path": form })).unwrap()).is_empty(), "nothing left to say");

    // A thin one still counts as none; clearing puts the warning back.
    let thin = call(&mut tools, "set_definition", json!({ "path": form, "element": "control", "id": "BTN-OK", "text": "Save" })).unwrap();
    assert_eq!(thin["thin"], true);
    assert_eq!(warning_about_definitions(&call(&mut tools, "check", json!({ "path": form })).unwrap()).len(), 1);
    call(&mut tools, "set_definition", json!({ "path": form, "element": "control", "id": "BTN-OK", "text": "" })).unwrap();
    let cleared = call(&mut tools, "describe", json!({ "path": form })).unwrap();
    assert_eq!(cleared["controls"].as_array().unwrap().iter().find(|c| c["id"] == "BTN-OK").unwrap()["definition"], "");
    println!("definition for AI over MCP: describe overview + file, set_definition form/control, check warns once per file, thin and cleared texts count as none");
}

#[test]
fn a_definition_does_not_change_the_program_that_is_generated() {
    let dir = fixture("generated");
    let mut tools = ProjectTools::new(HeadlessHost::new(&dir, "test"));
    let form = "forms/MAIN-FORM.cfrm";
    call(&mut tools, "regenerate", json!({ "path": form })).unwrap();
    let read = |dir: &Path| std::fs::read_to_string(std::fs::read_dir(dir.join("generated")).unwrap().flatten().next().unwrap().path()).unwrap();
    let plain = read(&dir);
    call(&mut tools, "set_definition", json!({ "path": form, "element": "form", "text": "The main screen." })).unwrap();
    call(&mut tools, "set_definition", json!({ "path": form, "element": "control", "id": "BTN-OK", "text": "Confirms and saves the customer data." })).unwrap();
    call(&mut tools, "regenerate", json!({ "path": form })).unwrap();
    assert_eq!(read(&dir), plain, "the generated COBOL is the same with definitions");
}

#[test]
fn what_cannot_be_written_is_refused_with_a_reason() {
    let dir = fixture("refused");
    let mut tools = ProjectTools::new(HeadlessHost::new(&dir, "test"));
    let form = "forms/MAIN-FORM.cfrm";
    let mut err = |args: Value| call(&mut tools, "set_definition", args).unwrap_err();
    assert!(err(json!({ "path": form, "element": "control", "id": "NOPE", "text": "x" })).contains("no control 'NOPE'"));
    assert!(err(json!({ "path": form, "element": "control", "text": "x" })).contains("needs its 'id'"));
    assert!(err(json!({ "path": form, "element": "field", "id": "A", "text": "x" })).contains("not an element of a form"));
    assert!(err(json!({ "path": form, "element": "layer", "id": "NOPE", "text": "x" })).contains("no layer"));
    assert!(err(json!({ "path": "forms/ABSENT.cfrm", "element": "form", "text": "x" })).contains("does not exist"));
    assert!(err(json!({ "path": "../outside.cfrm", "element": "form", "text": "x" })).len() > 3);
}

#[test]
fn a_sql_connection_definition_is_written_into_the_project_file_and_nothing_else_moves() {
    let dir = fixture("sql");
    let manifest = dir.join("CheckDemo.project.toml");
    let original = std::fs::read_to_string(&manifest).unwrap();
    let with_sql = format!("# the developer's note\n{original}\n[[sql-connections]]\nname = \"SALES\"\npath = \"data/sales.db\"\ndefault = true\n");
    std::fs::write(&manifest, &with_sql).unwrap();
    let mut tools = ProjectTools::new(HeadlessHost::new(&dir, "test"));

    let text = "The sales database: one row per invoice line, amounts in cents, read-only for reports.";
    let r = call(&mut tools, "set_definition", json!({ "path": "CheckDemo.project.toml", "element": "sql-connection", "id": "sales", "text": text })).unwrap();
    assert_eq!(r["thin"], false);
    let written = std::fs::read_to_string(&manifest).unwrap();
    assert!(written.starts_with("# the developer's note\n"), "the developer's comment stays");
    assert!(written.contains(&format!("definition = \"{text}\"")));
    assert_eq!(written.replace(&format!("definition = \"{text}\"\n"), ""), with_sql, "only the one key was added");

    let overview = call(&mut tools, "describe", json!({})).unwrap();
    assert_eq!(overview["sql_connections"][0]["name"], "SALES");
    assert_eq!(overview["sql_connections"][0]["definition"], text);

    call(&mut tools, "set_definition", json!({ "path": "CheckDemo.project.toml", "element": "sql-connection", "id": "SALES", "text": "" })).unwrap();
    assert_eq!(std::fs::read_to_string(&manifest).unwrap(), with_sql, "an empty text clears it");
    let e = call(&mut tools, "set_definition", json!({ "path": "CheckDemo.project.toml", "element": "sql-connection", "id": "NOPE", "text": "x" })).unwrap_err();
    assert!(e.contains("no SQL connection 'NOPE'"));
    println!("sql-connection definition: written, comment kept, described, cleared, unknown name refused");
}
