// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 084 R73 — what the IDE offers the developer, each mapped to the MCP
//! tool that offers it to a coding agent. The matches have no wildcard on
//! purpose: a new menu entry or designer operation does not compile until it
//! says which tool reaches it, and the test proves each named tool is served.
//! What no tool reaches yet is listed in [`GAPS`], with the reason.

// The map is read by its tests; the matches are the compile-time guard.
#![cfg_attr(not(test), allow(dead_code))]

use crate::agent::AgentOp;
use crate::menu_layout::Item;

/// The tool that does what a menu entry does.
pub fn menu_tool(item: Item) -> &'static str {
    match item {
        Item::NewProject => "create_project",
        Item::OpenProject => "open_project",
        Item::SaveProject | Item::Save => "save_project",
        Item::PackageProject => "package_project",
        Item::BuildBinary => "build",
        Item::DebugSettings | Item::PlatformSdkLocation | Item::BuildFolder => "set_ide_settings",
        Item::ReindexKnowledgeBases => "project_kb_reindex",
        Item::Quit
        | Item::ConfigureClaudeCode
        | Item::CodingAgentSettings
        | Item::Documentation
        | Item::Examples
        | Item::IdeWalkthrough
        | Item::TaskWizard
        | Item::WhatsNew
        | Item::About => "ide_command",
    }
}

/// The tool that applies a Form Designer operation; `None` for what changes
/// nothing (a message to the developer).
pub fn designer_op_tool(op: &AgentOp) -> Option<&'static str> {
    match op {
        AgentOp::DeployControl { .. }
        | AgentOp::CreateForm { .. }
        | AgentOp::SetProperty { .. }
        | AgentOp::GenerateEventHandler { .. }
        | AgentOp::CreateProcedure { .. }
        | AgentOp::SetFormStructure { .. }
        | AgentOp::AddLayer { .. }
        | AgentOp::RenameLayer { .. }
        | AgentOp::DeleteLayer { .. }
        | AgentOp::MoveLayer { .. }
        | AgentOp::SetLayerProperty { .. }
        | AgentOp::MoveToLayer { .. }
        | AgentOp::SetMenuItems { .. } => Some("apply_change_set"),
        AgentOp::Message { .. } => None,
    }
}

/// The IDE's panes, windows and places, and the tools that read or drive them.
pub const PANES: &[(&str, &[&str])] = &[
    ("Code editor", &["read_file", "edit_file", "write_file", "beautify", "ide_command"]),
    ("Project tree", &["list_files", "list_folder", "rename_file", "remove_file", "import_file", "add_to_project"]),
    ("Code search", &["search_files"]),
    ("Form Designer", &["apply_change_set", "describe", "render_form", "ide_command"]),
    ("Run Form", &["run_form"]),
    ("Indexed definition editor", &["edit_indexed", "set_definition", "validate"]),
    ("Project upgrade on open", &["upgrade_project"]),
    ("Indexed record grid", &["indexed_records"]),
    ("Project Settings", &["project_settings", "set_project_settings"]),
    ("Project crates", &["add_crate", "remove_crate"]),
    ("SQL Connections", &["sql_connections", "set_sql_connection", "remove_sql_connection", "test_sql_connection"]),
    ("Output", &["read_output"]),
    ("Problems and the build log", &["read_diagnostics", "check", "regenerate", "build"]),
    ("Run and Stop", &["run_project", "run_status", "stop_run"]),
    ("Backend window", &["start_backend", "backend_status", "stop_backend"]),
    ("Web Preview", &["start_web_preview"]),
    ("Run on Device", &["run_mobile", "create_android_studio_project"]),
    ("Debugger", &["debug_start", "debug_breakpoints", "debug_control", "debug_state"]),
    ("Project Knowledge Base", &["project_kb_search", "project_kb_reindex"]),
    ("Application Knowledge Base", &["app_kb_search"]),
    ("System Knowledge Base", &["kb_lookup", "kb_search"]),
    ("Version control", &["git"]),
    ("IDE-wide settings", &["ide_settings", "set_ide_settings"]),
    ("The IDE window", &["ide_state", "ide_picture", "ide_command", "ide_widget"]),
    ("PowerChat", &["add_powerchat"]),
    ("Gap reports", &["write_gap_report"]),
];

/// What the IDE offers that no tool reaches yet, and why.
pub const GAPS: &[(&str, &str)] = &[
    ("Run on the iOS simulator", "the IDE itself has no run on the iOS simulator yet; run_mobile answers so"),
    (
        "Grace and the IDE's own AI agents",
        "out of scope by the developer's decision (2026-11-18): the connection serves the IDE, not its assistant",
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    fn served() -> Vec<String> {
        cobolt_project_tools::ProjectTools::<cobolt_project_tools::HeadlessHost>::tool_list()
            .into_iter()
            .map(|t| t.name)
            .collect()
    }

    #[test]
    fn every_menu_entry_is_reached_by_a_served_tool() {
        let served = served();
        for item in Item::ALL {
            let tool = menu_tool(item);
            assert!(served.iter().any(|t| t == tool), "{item:?} maps to '{tool}', which no server lists");
        }
    }

    #[test]
    fn every_pane_and_designer_operation_is_reached_by_a_served_tool() {
        let served = served();
        for (pane, tools) in PANES {
            for tool in *tools {
                assert!(served.iter().any(|t| t == tool), "{pane} maps to '{tool}', which no server lists");
            }
        }
        let op = AgentOp::SetProperty { control_id: "A".into(), key: "Text".into(), value: serde_json::json!("x") };
        let tool = designer_op_tool(&op).unwrap();
        assert!(served.iter().any(|t| t == tool));
    }

    /// Every IDE tool appears somewhere in the map: nothing is served that the
    /// map does not account for.
    #[test]
    fn every_ide_tool_is_on_the_map() {
        let mapped: Vec<&str> = PANES
            .iter()
            .flat_map(|(_, t)| t.iter().copied())
            .chain(Item::ALL.iter().map(|i| menu_tool(*i)))
            .collect();
        for tool in cobolt_project_tools::tools::ide::tools() {
            assert!(mapped.contains(&tool.name.as_str()), "'{}' is served but not on the coverage map", tool.name);
        }
    }
}
