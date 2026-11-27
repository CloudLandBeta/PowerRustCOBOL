// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The tools that need the running IDE (spec 084 Phase D, R56–R64, R69–R72):
//! the Form Designer, Project Settings, run and debug, the IDE's own windows.
//! Both servers list them, so the agent sees one catalogue; each runs through
//! [`crate::host::ProjectHost::ide_tool`], which the IDE answers on its UI
//! thread and `rcrun mcp` answers with "open the IDE" (R66).

use cobolt_mcp::Tool;
use serde_json::{json, Value};

fn tool(name: &str, description: &str, properties: Value, required: &[&str]) -> Tool {
    Tool {
        name: name.into(),
        description: Some(description.into()),
        input_schema: json!({
            "type": "object",
            "properties": properties,
            "required": required,
            "additionalProperties": false,
        }),
        annotations: None,
    }
}

fn s(description: &str) -> Value {
    json!({ "type": "string", "description": description })
}

/// `package_project`: the one tool whose description and argument say which systems a Web package can be made for. `offered`
/// is [`System::offered`](cobolt_compiler::backend_runners::System::offered) of this installation's list of runners (spec 115
/// R2): the computer's own system, then every one the list has a runner for, spelled as `rcrun package --target` spells it.
fn package_project_tool(offered: &[cobolt_compiler::backend_runners::System]) -> Tool {
    let systems: Vec<String> = offered.iter().map(|s| s.to_string()).collect();
    let list = if systems.is_empty() { "none on this computer".to_owned() } else { systems.join(", ") };
    tool(
        "package_project",
        &format!(
            "Package the project as File ▸ Package Project does. A Desktop or Mobile project: a zip of its sources and \
             assets. A Web project: what is installed on a server, exactly two files in a folder: `<name>.zip` (the \
             page, the Backend and its production settings) and the install script, which takes the nginx folder as its \
             one required parameter. The package is for the system the IDE runs on unless `target` names another: \
             the systems this installation offers are {list}. The Backend of another system is the application's \
             payload joined to that system's runner, which must be the one the release published (it is checked); \
             `runner_dir` names a folder that holds it and `fetch_runner` allows the download from the release, which \
             is never made otherwise. A Backend that needs Cargo is packaged on the system it runs on, not for another. \
             The Backend file of another system is not run on this computer: the production settings and the module \
             are checked with this computer's own runner, and read_output says so. A Web package builds the page and \
             the Backend, so it answers at once; read_output says each step and ends with the command that installs. \
             Writes the files, replacing a package made before."
        ),
        json!({
            "to": s("Optional: an absolute, existing folder for the file(s); the project folder for a zip, `dist/install` for a Web package, when absent."),
            "target": s(&format!("Optional, a Web project only: the system the package is for, one of {list}; the system the IDE runs on when absent.")),
            "runner_dir": s("Optional, with `target`: an absolute, existing folder that holds the Backend runner of that system, as `<folder>/<system>/prc-backend-runner` and its bill of materials."),
            "fetch_runner": { "type": "boolean", "description": "Optional, with `target`: allow the download of the runner from the release when it is not on this computer. Default false: nothing is downloaded." },
        }),
        &[],
    )
}

/// Every IDE tool, in the order the catalogue shows them.
pub fn tools() -> Vec<Tool> {
    vec![
        // ── Forms (R56, R57) ────────────────────────────────────────────────
        tool(
            "apply_change_set",
            "Change a form as the Form Designer does, in one step that applies whole or not at all. \
             `operations` is a list of objects, each with an `op`: create_form {name, title?, width?, height?, folder?}; \
             deploy_control {control_type, id?, parent?, properties?, layer?}; set_property {control_id, key, value} \
             (geometry is X, Y, Width, Height); delete_control {control_id}; rename_control {control_id, new_id}; \
             generate_event_handler {control_id, event, code}; create_procedure {name, code}; \
             set_form_structure {block, code} (working-storage, file-control, file-section, special-names, linkage); \
             add_layer {name?}; rename_layer {name, new_name}; delete_layer {name}; move_layer {name, position}; \
             set_layer_property {layer, key, value}; move_to_layer {control_ids, layer}; \
             set_menu_items {control_id, items}. Every operation is validated first; if one is invalid nothing \
             changes and the answer names it. A form open in a designer shows the change at once and undoes it in \
             one step. Changes the form's file.",
            json!({
                "path": s("The form, project-relative, e.g. forms/MAIN.cfrm. For a change set that starts with create_form, the folder the new form goes in may be given there instead."),
                "operations": { "type": "array", "items": { "type": "object" }, "description": "The operations, in order." },
            }),
            &["operations"],
        ),
        // ── The project (R59–R61) ───────────────────────────────────────────
        tool(
            "project_settings",
            "Read every field of Project Settings, grouped as the Settings form groups them. A secret shows only \
             whether it is set. Read-only.",
            json!({}),
            &[],
        ),
        tool(
            "set_project_settings",
            "Change Project Settings fields through the IDE's project model, which saves and seals the project. \
             `values` maps a field name, as project_settings answers it, to its new value. A secret goes to the \
             IDE's secret store and is never answered back.",
            json!({ "values": { "type": "object", "description": "Field name → new value." } }),
            &["values"],
        ),
        tool(
            "sql_connections",
            "List the project's SQL connections: name, engine and where each points. Never a password. Read-only.",
            json!({}),
            &[],
        ),
        tool(
            "set_sql_connection",
            "Create or change a SQL connection of the project. `fields` holds what the SQL Connections window \
             holds (engine, host, port, database, user, file, options); `password`, when given, goes to the IDE's \
             secret store. Changes the project.",
            json!({
                "name": s("The connection's name, as EXEC SQL CONNECT names it."),
                "fields": { "type": "object", "description": "The connection's fields." },
                "password": s("Optional: the password, kept by the IDE and never answered back."),
            }),
            &["name"],
        ),
        tool(
            "remove_sql_connection",
            "Remove a SQL connection from the project (and its stored password). Changes the project.",
            json!({ "name": s("The connection's name.") }),
            &["name"],
        ),
        tool(
            "test_sql_connection",
            "Connect with a SQL connection as the SQL Connections window's Test does, and answer success or the \
             database's own error. Never a password.",
            json!({ "name": s("The connection's name.") }),
            &["name"],
        ),
        tool("save_project", "Save every open designer, editor tab and the project, as File ▸ Save Project does. Changes files.", json!({}), &[]),
        package_project_tool(&cobolt_compiler::backend_runners::System::offered(&cobolt_compiler::backend_runners::RunnerManifest::find())),
        tool(
            "upgrade_project",
            "Upgrade a project made by an older version of PowerRustCOBOL AI to this version's layout, as the IDE \
             offers when it opens one. Changes the project.",
            json!({}),
            &[],
        ),
        // ── Run (R62) ───────────────────────────────────────────────────────
        tool(
            "run_project",
            "Run the project as Run does: it regenerates, then starts the application. Answers at once; read its \
             output and whether it ended with run_status, and stop it with stop_run. Runs the developer's code with \
             its real effects.",
            json!({}),
            &[],
        ),
        tool("run_status", "The running application's output so far, whether it ended and its exit status. Read-only.", json!({}), &[]),
        tool("stop_run", "Stop the application run_project started.", json!({}), &[]),
        tool(
            "start_backend",
            "Start a Web or Mobile project's Backend as Run ▸ Start Backend does. Answers its address; \
             backend_status says whether it answers. Runs the developer's code.",
            json!({}),
            &[],
        ),
        tool("backend_status", "Whether the Backend runs, its address, whether it answers, and its latest log lines. Read-only.", json!({}), &[]),
        tool("stop_backend", "Stop the Backend start_backend started.", json!({}), &[]),
        tool(
            "start_web_preview",
            "Build a Web or Mobile project's page and serve it as the Web Preview does; answers its address.",
            json!({}),
            &[],
        ),
        tool(
            "run_mobile",
            "Run a Mobile project on an Android device or emulator, or on the iOS simulator, as Run does. \
             Answers at once; run_status follows it.",
            json!({
                "target": { "type": "string", "enum": ["android", "ios-simulator"], "description": "Where to run." },
                "device": s("Optional: the device or simulator name; the IDE's choice when absent."),
            }),
            &["target"],
        ),
        tool(
            "create_android_studio_project",
            "Create the Android Studio project of a Mobile project, as the IDE's Android Studio entry does. Writes files.",
            json!({}),
            &[],
        ),
        // ── Debugger (R63) ──────────────────────────────────────────────────
        tool(
            "debug_start",
            "Start the debugger on the project (or on one program or form), as Run ▸ Debug does; it stops at the \
             first breakpoint. The session shows in the IDE's debugger. Runs the developer's code.",
            json!({ "path": s("Optional: the program or form to debug, project-relative; the main form when absent.") }),
            &[],
        ),
        tool(
            "debug_breakpoints",
            "Set or clear breakpoints, by project-relative file and 1-based line, and answer the breakpoints now set.",
            json!({
                "set": { "type": "array", "items": { "type": "object" }, "description": "[{file, line}] to set." },
                "clear": { "type": "array", "items": { "type": "object" }, "description": "[{file, line}] to clear." },
                "clear_all": { "type": "boolean", "description": "Clear every breakpoint first." },
            }),
            &[],
        ),
        tool(
            "debug_control",
            "Drive the debug session: continue, pause, step_over, step_into, step_out or stop. Answers where it \
             stopped (debug_state's answer).",
            json!({ "action": { "type": "string", "enum": ["continue", "pause", "step_over", "step_into", "step_out", "stop"] } }),
            &["action"],
        ),
        tool(
            "debug_state",
            "Where the debug session is: running or stopped, file and line, the call stack, and the values of the \
             data items asked for (or of every watch). Read-only.",
            json!({ "variables": { "type": "array", "items": { "type": "string" }, "description": "Optional: data-item names to read." } }),
            &[],
        ),
        // ── Knowledge, data and the IDE's panes (R64) ───────────────────────
        tool(
            "project_kb_search",
            "Search the project's Knowledge Base (the documents in its Knowledge Base folder) and answer the matching \
             passages. Read-only.",
            json!({ "query": s("What to look for."), "limit": { "type": "integer", "description": "Most passages; 8 when absent." } }),
            &["query"],
        ),
        tool("project_kb_reindex", "Rebuild the project's Knowledge Base index from its documents, as the IDE's Reindex does.", json!({}), &[]),
        tool(
            "indexed_records",
            "Browse and change the records of an indexed file, as the IDE's record grid does. `action`: list, find \
             (by `key`), add, update or delete (`record` maps field names to values). Changes the data file for \
             add, update and delete.",
            json!({
                "path": s("The indexed definition (.cidx), project-relative."),
                "action": { "type": "string", "enum": ["list", "find", "add", "update", "delete"] },
                "key": s("For find, update and delete: the primary key's value."),
                "record": { "type": "object", "description": "For add and update: field name → value." },
                "limit": { "type": "integer", "description": "For list: most records; 50 when absent." },
            }),
            &["path", "action"],
        ),
        tool(
            "read_output",
            "The IDE's Output panel: its latest lines. Read-only.",
            json!({ "lines": { "type": "integer", "description": "How many of the latest lines; 200 when absent." } }),
            &[],
        ),
        tool(
            "read_diagnostics",
            "The IDE's diagnostics: the problems of the last check or build, and the latest lines of the error log. Read-only.",
            json!({ "lines": { "type": "integer", "description": "How many log lines; 100 when absent." } }),
            &[],
        ),
        tool(
            "beautify",
            "Lay a COBOL source of the project (.cbl, .cob, .cpy; not generated/) out as the editor's Beautify does, \
             with the developer's Beautify options. Refused when the result would not parse as the original did. \
             Changes the file.",
            json!({ "path": s("The source, project-relative, e.g. src/TOTALS.cbl.") }),
            &["path"],
        ),
        // ── IDE-wide settings (R69) ─────────────────────────────────────────
        tool(
            "ide_settings",
            "Read the IDE-wide settings: the IDE theme, the default form theme, the language, Debug Settings, the \
             Build Folder and the Platform SDK paths. Read-only.",
            json!({}),
            &[],
        ),
        tool(
            "set_ide_settings",
            "Change IDE-wide settings, by the names ide_settings answers. The tool server's port and access token \
             are not settable here. Changes the IDE's settings.",
            json!({ "values": { "type": "object", "description": "Setting name → new value." } }),
            &["values"],
        ),
        // ── The IDE itself (R70–R72) ────────────────────────────────────────
        tool(
            "ide_state",
            "What the IDE shows: the open project, the open tabs and windows and which is active, unsaved files, \
             the controls selected in each designer, and the panels shown. Read-only.",
            json!({}),
            &[],
        ),
        tool(
            "ide_picture",
            "A picture (PNG) of the IDE window as the developer sees it, or of one panel. Read-only.",
            json!({ "panel": s("Optional: output, project, properties, toolbox, debugger, or a designer's form path.") }),
            &[],
        ),
        tool(
            "ide_command",
            "Act on the IDE by a named operation: open_tab {path, line?}, activate_tab {path}, close_tab {path} \
             (refused while it holds unsaved changes), goto_line {path, line}, select_controls {path, ids}, \
             set_layer {path, layer}, show_panel {panel}, hide_panel {panel}, menu {entry} (e.g. \"File/Save \
             Project\", \"Run/Check\", \"Help/Developer's Guide\"). Answers what changed.",
            json!({
                "command": s("The operation's name."),
                "args": { "type": "object", "description": "The operation's arguments." },
            }),
            &["command"],
        ),
        tool(
            "ide_widget",
            "For what no ide_command covers: list the IDE's widgets (`tree`), or point at, click, type into or \
             scroll a widget found by its label or id. Never answers a dialog that would discard unsaved work.",
            json!({
                "action": { "type": "string", "enum": ["tree", "click", "type", "scroll"] },
                "target": s("For click, type and scroll: the widget's label or id, as tree answers it."),
                "text": s("For type: the text."),
                "dx": { "type": "number" },
                "dy": { "type": "number" },
            }),
            &["action"],
        ),
    ]
}

/// Whether `name` is an IDE tool.
pub fn is_ide_tool(name: &str) -> bool {
    tools().iter().any(|t| t.name == name)
}

/// IDE tools that need no project open: the IDE-wide ones and the IDE's own
/// windows and panes.
pub fn project_free(name: &str) -> bool {
    matches!(
        name,
        "ide_settings" | "set_ide_settings" | "ide_state" | "ide_picture" | "ide_command" | "ide_widget" | "read_output" | "read_diagnostics"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use cobolt_compiler::backend_runners::{Arch, Os, RunnerManifest, System};
    use std::time::Instant;

    const LINUX: System = System { os: Os::Linux, arch: Arch::X86_64 };

    /// Spec 115 T11 (R2, AC8): the tool names the systems of the one list, and its three arguments are in its schema.
    #[test]
    fn package_project_names_the_systems_of_the_one_list_and_takes_three_arguments() {
        let began = Instant::now();
        let list = RunnerManifest::parse(&format!("{}  {LINUX}/prc-backend-runner\n{}  {LINUX}/prc-backend-runner.sbom.json\n", "a".repeat(64), "b".repeat(64)));
        let offered = System::offered(&list);
        let tool = package_project_tool(&offered);
        let words: Vec<String> = offered.iter().map(System::to_string).collect();
        assert!(words.contains(&LINUX.to_string()), "{words:?}");
        let description = tool.description.clone().unwrap();
        let properties = &tool.input_schema["properties"];
        for word in &words {
            assert!(description.contains(word.as_str()), "the description names {word}");
            assert!(properties["target"]["description"].as_str().unwrap().contains(word.as_str()), "the argument names {word}");
        }
        let mut keys: Vec<&str> = properties.as_object().unwrap().keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(keys, ["fetch_runner", "runner_dir", "target", "to"]);
        assert_eq!(properties["fetch_runner"]["type"], "boolean");
        assert_eq!(tool.input_schema["required"], json!([]), "no argument is required: no choice is today's behaviour");
        // With no list of runners, only this computer's own system is offered, and the tool says so.
        let own = package_project_tool(&System::offered(&RunnerManifest::default()));
        let own_words: Vec<String> = System::offered(&RunnerManifest::default()).iter().map(System::to_string).collect();
        assert!(own_words.len() <= 1 && own_words.iter().all(|w| own.description.clone().unwrap().contains(w.as_str())));
        // The catalogue holds the tool once, under its name.
        assert_eq!(tools().iter().filter(|t| t.name == "package_project").count(), 1);
        println!("package_project: {} systems named from the list ({}), 4 arguments, none required; {} ms", words.len(), words.join(", "), began.elapsed().as_millis());
    }
}
