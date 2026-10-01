// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The seven tools (spec 080 plan §1.6) and the MCP front of them.
//!
//! Every answer is one text block holding compact JSON, so an agent can parse
//! it. Every path argument is project-relative and goes through
//! [`crate::root::ProjectRoot::resolve`]. No tool reads or writes anything
//! outside the project: not the IDE's settings, not API keys, not the SDK
//! (R13).

use std::sync::{Arc, Mutex};
use std::time::Duration;

use cobolt_mcp::{Content, McpHandler, ServerInfo, Tool, ToolResult};
use serde_json::{json, Value};

use crate::host::ProjectHost;
use crate::root::ProjectRoot;

pub mod build;
pub mod check;
pub mod kb;
pub mod list;
pub mod regenerate;
pub mod register;
pub mod validate;

/// What every handler serving one project shares: the lock that serialises
/// writing tools, and the build in flight (so a repeated `build` joins it
/// instead of starting a second one). One `Shared` per server; the HTTP
/// transport hands the same one to every connection.
pub struct Shared {
    pub(crate) write_lock: Mutex<()>,
    pub(crate) build: build::BuildSlot,
    pub(crate) builder: build::Builder,
    pub(crate) build_wait: Duration,
}

impl Shared {
    /// The real builder (`cobolt_compiler::build_project`), waiting at most
    /// [`build::DEFAULT_BUILD_WAIT`] per call.
    pub fn new() -> Self {
        Self::with_builder(build::compiler_builder(), build::DEFAULT_BUILD_WAIT)
    }

    /// A shared state with another builder and wait — tests use a stub.
    pub fn with_builder(builder: build::Builder, build_wait: Duration) -> Self {
        Self {
            write_lock: Mutex::new(()),
            build: build::BuildSlot::default(),
            builder,
            build_wait,
        }
    }
}

impl Default for Shared {
    fn default() -> Self {
        Self::new()
    }
}

/// The tool set over one host's project.
pub struct ProjectTools<H: ProjectHost> {
    host: H,
    shared: Arc<Shared>,
}

impl<H: ProjectHost> ProjectTools<H> {
    /// Tools with their own shared state (one stdio session).
    pub fn new(host: H) -> Self {
        Self::with_shared(host, Arc::new(Shared::new()))
    }

    /// Tools that share state with other handlers of the same server.
    pub fn with_shared(host: H, shared: Arc<Shared>) -> Self {
        Self { host, shared }
    }

    pub fn host(&self) -> &H {
        &self.host
    }

    pub fn host_mut(&mut self) -> &mut H {
        &mut self.host
    }

    /// The tools on offer — static, so a client can list them before a
    /// project is open.
    pub fn tool_list() -> Vec<Tool> {
        let path_arg = |desc: &str| json!({ "type": "string", "description": desc });
        let none = json!({ "type": "object", "properties": {}, "additionalProperties": false });
        vec![
            Tool {
                name: "list_files".into(),
                description: Some(
                    "List the project's files as its manifest tracks them (forms, indexed, sources, \
                     generated, assets, documentation), each with whether it exists on disk, plus the \
                     gap reports in docs/compiler-requests/. Read-only."
                        .into(),
                ),
                input_schema: none.clone(),
            },
            Tool {
                name: "check".into(),
                description: Some(
                    "Check the whole project, or one file, exactly as the IDE's Check and Build gate do: \
                     every form's generated program (validated in memory, nothing written), every \
                     source, every .cfrm/.cidx, the main-form designation and data-binding blockers. \
                     A form diagnostic names the .cfrm, the code site (control ▸ event) and the line \
                     inside that handler. Read-only."
                        .into(),
                ),
                input_schema: json!({
                    "type": "object",
                    "properties": { "path": path_arg("Optional project-relative .cfrm, .cidx or COBOL source; omit for the whole project.") },
                    "additionalProperties": false
                }),
            },
            Tool {
                name: "regenerate".into(),
                description: Some(
                    "Regenerate the COBOL of one form (.cfrm) or indexed-file definition (.cidx), or of \
                     all of them, with the IDE's own generator, write it where the IDE writes it and \
                     record it in the manifest as generated. Never edit generated .cbl by hand."
                        .into(),
                ),
                input_schema: json!({
                    "type": "object",
                    "properties": { "path": path_arg("Optional project-relative .cfrm or .cidx; omit for all.") },
                    "additionalProperties": false
                }),
            },
            Tool {
                name: "add_to_project".into(),
                description: Some(
                    "Add an existing project file to its manifest list (routed by extension, or the \
                     list named), re-sealing the main-form designation as the IDE does. The only way to \
                     add a form or indexed file: never edit the project manifest by hand."
                        .into(),
                ),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "path": path_arg("Project-relative path of the file to add."),
                        "list": { "type": "string", "enum": ["forms", "indexed", "sources", "assets", "documentation"],
                                  "description": "Optional manifest list; by default chosen from the extension." }
                    },
                    "required": ["path"],
                    "additionalProperties": false
                }),
            },
            Tool {
                name: "build".into(),
                description: Some(
                    "Regenerate everything, refuse on any check error, then build the binary. Waits at \
                     most 40 seconds: if the build is still running it answers {\"status\":\"running\"} \
                     — call build again to keep waiting on the same build."
                        .into(),
                ),
                input_schema: json!({
                    "type": "object",
                    "properties": { "full": { "type": "boolean", "description": "Discard cached build artefacts first." } },
                    "additionalProperties": false
                }),
            },
            Tool {
                name: "validate".into(),
                description: Some(
                    "Validate one .cfrm (it loads) or .cidx (it loads, its structure is valid; warnings \
                     listed). Read-only."
                        .into(),
                ),
                input_schema: json!({
                    "type": "object",
                    "properties": { "path": path_arg("Project-relative .cfrm or .cidx.") },
                    "required": ["path"],
                    "additionalProperties": false
                }),
            },
            Tool {
                name: "kb_lookup".into(),
                description: Some(
                    "Look a control, property, method, event or built-in up in the PowerRustCOBOL \
                     reference this binary carries. A name it does not find does not exist: write a gap \
                     report instead of inventing it."
                        .into(),
                ),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "name": { "type": "string", "description": "e.g. Button, Caption, onClick, AddRow, HTTP-GET" },
                        "kind": { "type": "string", "enum": ["control", "property", "method", "event", "builtin"],
                                  "description": "Optional: narrow the lookup." }
                    },
                    "required": ["name"],
                    "additionalProperties": false
                }),
            },
        ]
    }

    /// Run one tool by name. `Ok` is the JSON answer; `Err` is a tool-level
    /// failure, said in words.
    pub fn call(&mut self, name: &str, args: &Value) -> Result<Value, String> {
        if !Self::tool_list().iter().any(|t| t.name == name) {
            return Err(format!("no such tool: {name}"));
        }
        let root: ProjectRoot = self.host.project().map_err(|e| e.message().to_owned())?;
        let shared = Arc::clone(&self.shared);
        match name {
            "list_files" => list::run(&root),
            "check" => check::run(&self.host, &root, opt_str(args, "path").as_deref()),
            "regenerate" => {
                let _w = shared.write_lock.lock().unwrap_or_else(|p| p.into_inner());
                regenerate::run(&mut self.host, &root, opt_str(args, "path").as_deref())
            }
            "add_to_project" => {
                let path = opt_str(args, "path").ok_or("add_to_project needs a 'path'")?;
                let _w = shared.write_lock.lock().unwrap_or_else(|p| p.into_inner());
                register::run(&mut self.host, &root, &path, opt_str(args, "list").as_deref())
            }
            "build" => build::run(
                &mut self.host,
                &root,
                &shared,
                args.get("full").and_then(Value::as_bool).unwrap_or(false),
            ),
            "validate" => {
                let path = opt_str(args, "path").ok_or("validate needs a 'path'")?;
                validate::run(&root, &path)
            }
            "kb_lookup" => {
                let n = opt_str(args, "name").ok_or("kb_lookup needs a 'name'")?;
                kb::run(&n, opt_str(args, "kind").as_deref())
            }
            _ => unreachable!("listed above"),
        }
    }
}

fn opt_str(args: &Value, key: &str) -> Option<String> {
    args.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

/// The MCP front: every method delegates, so this impl holds no logic.
impl<H: ProjectHost> McpHandler for ProjectTools<H> {
    fn server_info(&self) -> ServerInfo {
        ServerInfo {
            name: "PowerRustCOBOL".into(),
            version: self.host.version(),
        }
    }

    fn list_tools(&mut self) -> Vec<Tool> {
        Self::tool_list()
    }

    fn call_tool(&mut self, name: &str, arguments: &Value) -> ToolResult {
        match self.call(name, arguments) {
            Ok(v) => ToolResult::ok(vec![Content::text(v.to_string())]),
            Err(e) => ToolResult::failed(e),
        }
    }
}
