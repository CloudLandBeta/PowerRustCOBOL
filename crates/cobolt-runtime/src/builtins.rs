// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The RustCOBOL built-in CALLs — the `COBOL` object's methods.
//!
//! Each is reached two ways, and they are the same call:
//!
//! ```cobol
//!            COBOL::"MODEL-LIST" ( WS-PROV-ID WS-CUR-URL WS-KEY WS-N )
//!            CALL "COBOL-MODEL-LIST" USING WS-PROV-ID WS-CUR-URL WS-KEY WS-N
//! ```
//!
//! The inline form is the one to write (operator, 2026-09-28). This table
//! is the one list of them the editor's completion, the agents' prompts and
//! the documentation draw on; `the_table_is_every_builtin_the_interpreter_dispatches`
//! keeps it equal to what `exec_call` actually handles.

/// One built-in: its name WITHOUT the `COBOL-` prefix (the method name), its
/// arguments in order — each marked `in`, `out` or `in-out` — and what it does.
#[derive(Debug, Clone, Copy)]
pub struct Builtin {
    pub name: &'static str,
    pub params: &'static str,
    pub description: &'static str,
}

const fn b(name: &'static str, params: &'static str, description: &'static str) -> Builtin {
    Builtin { name, params, description }
}

/// Every built-in, alphabetically.
pub const BUILTINS: &[Builtin] = &[
    b("APPEND-FILE", "path in, text in, [status out]", "Append a line of text to a file, creating it if missing"),
    b("BINDING-LOAD", "binding-id in, status out", "Load a data binding's records from its source"),
    b("BINDING-MARK-CLEAN", "binding-id in, dirty-flag out", "Mark a binding clean: no pending edits"),
    b("BINDING-POPULATE", "binding-id in, status out", "Fill a binding's bound controls from its data"),
    b("BINDING-SET-PENDING", "binding-id in, row-key in, value in, dirty-flag out", "Record a pending edit for one row of a binding"),
    b("BINDING-SET-READ-ONLY", "binding-id in, flag in", "Make a binding read-only (flag not \"0\") or writable"),
    b("BINDING-UPDATE", "binding-id in, row-key in, status out", "Write a binding row's pending changes back to its source"),
    b("CHART-ADD-POINT", "chart-id in, label in, value in, [more values in]", "Add one point to a chart"),
    b("CHART-CLEAR", "chart-id in", "Remove all data from a chart"),
    b("CHART-REFRESH", "chart-id in", "Repaint a chart from its current data"),
    b("CHART-SET-TABLE", "chart-id in, table in, count in", "Replace a chart's data with count rows of a table"),
    b("CLOSE-DB", "handle in", "Close a database connection"),
    b("EXEC-SQL", "handle in, sql in, row-count out, status out", "Run a SQL statement; returns the row or affected count"),
    b("FETCH-ROW", "handle in, column in, value out, status out", "Read one column (1-based) of the current result row"),
    b("FILE-STATUS", "file-name in, status out", "Copy a file's last FILE STATUS code into a data item"),
    b("FOLDER-DIALOG", "title in, [start-folder in], path out", "Ask the operator for a folder; spaces when cancelled"),
    b("GET-PROPERTY", "object in, property in, value out", "Read a control's property into a data item"),
    b("HTTP-CLEAR-HEADERS", "", "Remove every header set with HTTP-SET-HEADER"),
    b("HTTP-DELETE", "url in, response out, http-status out", "Send an HTTP DELETE; returns the body and status code"),
    b("HTTP-GET", "url in, response out, http-status out", "Send an HTTP GET; returns the body and status code"),
    b("HTTP-POST", "url in, body in, response out, http-status out", "Send an HTTP POST (JSON by default); returns body and status"),
    b("HTTP-PUT", "url in, body in, response out, http-status out", "Send an HTTP PUT; returns the body and status code"),
    b("HTTP-SET-HEADER", "name in, value in", "Add or replace a header sent on every later HTTP call"),
    b("INIT-FORM", "[form-name in]", "Initialise the form (generated code calls it)"),
    b("KEY-IS-SET", "entry in, flag out", "Whether an API key is stored for a model entry: Y or N"),
    b("KEY-REMOVE", "entry in, [status out]", "Delete the stored API key of a model entry"),
    b("KEY-SET", "entry in, key in, [status out]", "Store the API key of a model entry (never read back)"),
    b("MCP-SEARCH", "tool in, arguments-json in, result out", "Call an MCP tool with JSON arguments; returns its text"),
    b("MODEL-LIST", "provider in, endpoint in, key in, count out, status out, [entry in]", "Ask a provider which models it offers"),
    b("MODEL-LIST-GET", "index in, model out", "One model name (1-based) from the last MODEL-LIST"),
    b("MODEL-REMOVE", "entry in, [status out]", "Remove an entry from the model list"),
    b("MODEL-SET", "entry in, api in, url in, model in, [status out]", "Add or replace an entry in the model list"),
    b("MODEL-TEST", "provider in, endpoint in, model in, key in, [status out], [entry in]", "Send a model a tiny request to test the connection"),
    b("NEXT-ROW", "handle in, more out", "Move to the next result row: Y when there is one, N at the end"),
    b("OPEN-DB", "connection-string in, handle out, status out", "Open a SQLite, PostgreSQL or MySQL connection"),
    b("OPEN-FILE-DIALOG", "title in, [filter in], [start-folder in], path out", "Ask the operator for a file to open; spaces when cancelled"),
    b("PROVIDER-COUNT", "count out", "How many model providers there are"),
    b("PROVIDER-GET", "index in, [id out], [label out], [endpoint out], [needs-key out]", "One provider's details (1-based)"),
    b("ROW-COUNT", "handle in, count out", "How many rows the last result set has"),
    b("SAVE-FILE-DIALOG", "title in, [filter in], [file-name in], [start-folder in], path out", "Ask the operator where to save; spaces when cancelled"),
    b("SET-PROPERTY", "object in, property in, value in", "Set a control's property"),
    b("WAIT-EVENT", "event-id out, control-id out", "Wait for the next event (generated code calls it)"),
    b("WRITE-FILE", "path in, text in, [status out]", "Write a file holding one line of text, replacing it"),
];

/// The built-in `name` names — with or without its `COBOL-` prefix, any case.
pub fn builtin(name: &str) -> Option<&'static Builtin> {
    let up = name.trim().trim_matches('"').to_ascii_uppercase();
    let bare = up.strip_prefix("COBOL-").unwrap_or(&up);
    BUILTINS.iter().find(|b| b.name == bare)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The table and the interpreter's dispatch name the same built-ins: a
    /// name added to one and not the other is a completion that calls
    /// nothing, or a built-in nobody is told about.
    #[test]
    fn the_table_is_every_builtin_the_interpreter_dispatches() {
        let src = include_str!("interpreter.rs");
        let mut dispatched: Vec<String> = Vec::new();
        for (i, _) in src.match_indices("\"COBOL-") {
            let rest = &src[i + 1..];
            let end = rest.find('"').unwrap_or(0);
            let name = &rest[..end];
            let after = rest[end + 1..].trim_start();
            // A match arm: `"COBOL-X" =>`, `"COBOL-X" |`, `"COBOL-X" if …`,
            // or a tuple arm of the file dialogs.
            if (after.starts_with("=>") || after.starts_with('|') || after.starts_with("if "))
                && name.chars().all(|c| c.is_ascii_uppercase() || c == '-')
            {
                dispatched.push(name.trim_start_matches("COBOL-").to_string());
            }
        }
        dispatched.sort();
        dispatched.dedup();
        let listed: Vec<String> = BUILTINS.iter().map(|b| b.name.to_string()).collect();
        let missing: Vec<&String> = dispatched.iter().filter(|d| !listed.contains(d)).collect();
        let phantom: Vec<&String> = listed.iter().filter(|l| !dispatched.contains(l)).collect();
        println!("  {} built-ins dispatched, {} listed", dispatched.len(), listed.len());
        assert!(missing.is_empty(), "dispatched but not in BUILTINS: {missing:?}");
        assert!(phantom.is_empty(), "in BUILTINS but not dispatched: {phantom:?}");
        let mut sorted = listed.clone();
        sorted.sort();
        assert_eq!(listed, sorted, "BUILTINS stays alphabetical");
        assert_eq!(builtin("cobol-model-list").map(|b| b.name), Some("MODEL-LIST"));
        assert_eq!(builtin("\"PROVIDER-COUNT\"").map(|b| b.name), Some("PROVIDER-COUNT"));
    }
}
