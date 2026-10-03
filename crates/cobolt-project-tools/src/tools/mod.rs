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
pub mod render;
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
    /// Pictures a form (`render_form`); `None` in a host that cannot.
    pub(crate) renderer: Option<render::Renderer>,
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
            renderer: None,
        }
    }

    /// Give `render_form` its renderer (spec 084 R30).
    pub fn with_renderer(mut self, renderer: render::Renderer) -> Self {
        self.renderer = Some(renderer);
        self
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
            Tool {
                name: "render_form".into(),
                description: Some(
                    "A picture (PNG) of a form as Run Form draws it when it opens — its theme, \
                     backdrop, controls and images — so you can see what you built: layout, \
                     overlaps, text that does not fit. Before any event handler runs."
                        .into(),
                ),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Project-relative .cfrm, e.g. forms/ORDERS.cfrm" },
                        "scale": { "type": "number", "minimum": 0.25, "maximum": 3,
                                   "description": "Picture scale, 1 = one pixel per designed point (default 1)." }
                    },
                    "required": ["path"],
                    "additionalProperties": false
                }),
            },
            Tool {
                name: "create_project".into(),
                description: Some(
                    "Create a new PowerRustCOBOL project in an empty or new folder — exactly what the \
                     IDE's New Project makes: the manifest, the standard folders and a runnable main \
                     program — and make it the project these tools act on. Refuses a folder that \
                     already holds files."
                        .into(),
                ),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "folder": { "type": "string", "description": "Absolute path of the new project's folder." },
                        "name": { "type": "string", "description": "The project's name, e.g. Inventory." }
                    },
                    "required": ["folder", "name"],
                    "additionalProperties": false
                }),
            },
            Tool {
                name: "open_project".into(),
                description: Some(
                    "Open an existing PowerRustCOBOL project — its folder or its manifest — as the project \
                     these tools act on (in PowerRustCOBOL AI, as File → Open Project does)."
                        .into(),
                ),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Absolute path of the project folder or its .project.toml." }
                    },
                    "required": ["path"],
                    "additionalProperties": false
                }),
            },
            Tool {
                name: "kb_search".into(),
                description: Some(
                    "Search the PowerRustCOBOL Knowledge Base in free text — the same store the IDE's \
                     assistant searches — when you do not know the exact name: 'navigation rail with forms \
                     in a pane', 'read an indexed file backwards'. Returns the best-matching subjects with \
                     their text. Use kb_lookup when you know the name."
                        .into(),
                ),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "Words to search for." },
                        "limit": { "type": "integer", "minimum": 1, "maximum": 25,
                                   "description": "Most subjects to return (default 8)." }
                    },
                    "required": ["query"],
                    "additionalProperties": false
                }),
            },
        ]
    }

    /// The tool list, every PROJECT tool also taking the optional `project`
    /// argument (spec 084 R20). The knowledge tools need no project.
    pub fn tool_list_with_project() -> Vec<Tool> {
        let mut tools = Self::tool_list();
        for tool in tools.iter_mut().filter(|t| !project_free(&t.name)) {
            if let Some(props) = tool.input_schema.get_mut("properties").and_then(Value::as_object_mut) {
                props.insert(
                    "project".into(),
                    json!({ "type": "string", "description": "Optional: the project this call is for — its folder (your working directory), its manifest or its name. A call for any project other than the one open is refused." }),
                );
            }
        }
        tools
    }

    /// Run one tool by name. `Ok` is the JSON answer; `Err` is a tool-level
    /// failure, said in words.
    pub fn call(&mut self, name: &str, args: &Value) -> Result<Value, String> {
        if !Self::tool_list().iter().any(|t| t.name == name) {
            return Err(format!("no such tool: {name}"));
        }
        // Knowledge needs no open project (spec 084): answered before the
        // project is resolved.
        match name {
            "kb_lookup" => {
                let n = opt_str(args, "name").ok_or("kb_lookup needs a 'name'")?;
                return kb::run(&n, opt_str(args, "kind").as_deref());
            }
            "kb_search" => {
                let q = opt_str(args, "query").ok_or("kb_search needs a 'query'")?;
                return kb::search(&q, args.get("limit").and_then(Value::as_u64));
            }
            // Spec 084 R15/R16 — they make or choose the project, so none need be open.
            "create_project" => {
                let folder = absolute_arg(args, "folder", "create_project")?;
                let name = opt_str(args, "name").ok_or("create_project needs a 'name'")?;
                return self.host.create_project(&folder, &name);
            }
            "open_project" => {
                let path = absolute_arg(args, "path", "open_project")?;
                return self.host.open_project(&path);
            }
            _ => {}
        }
        let root: ProjectRoot = self.host.project().map_err(|e| e.message().to_owned())?;
        // Spec 084 R20: a call for another project is refused, never applied
        // to the one that happens to be open.
        if let Some(asked) = opt_str(args, "project") {
            if !names_project(&root, &asked) {
                return Err(format!(
                    "this call is for project '{asked}', but the project open is '{}'. \
                     Open the right one with open_project, or ask the developer.",
                    root.manifest_name()
                ));
            }
        }
        let shared = Arc::clone(&self.shared);
        let answer = self.run_project_tool(name, args, &root, &shared)?;
        // Spec 084 R19: every answer names the project it acted on.
        Ok(match answer {
            Value::Object(mut map) => {
                map.entry("project").or_insert_with(|| Value::String(root.manifest_name()));
                Value::Object(map)
            }
            other => json!({ "project": root.manifest_name(), "result": other }),
        })
    }

    /// `render_form` with its picture: the project checks of [`Self::call`],
    /// then the PNG and what the agent is told about it.
    pub fn render_form(&mut self, args: &Value) -> Result<(Vec<u8>, Value), String> {
        let root: ProjectRoot = self.host.project().map_err(|e| e.message().to_owned())?;
        if let Some(asked) = opt_str(args, "project") {
            if !names_project(&root, &asked) {
                return Err(format!(
                    "this call is for project '{asked}', but the project open is '{}'. \
                     Open the right one with open_project, or ask the developer.",
                    root.manifest_name()
                ));
            }
        }
        let (png, mut meta) = self.render(args, &root)?;
        meta["project"] = json!(root.manifest_name());
        Ok((png, meta))
    }

    fn render(&self, args: &Value, root: &ProjectRoot) -> Result<(Vec<u8>, Value), String> {
        let path = opt_str(args, "path").ok_or("render_form needs a 'path'")?;
        let scale = args.get("scale").and_then(Value::as_f64).unwrap_or(1.0).clamp(0.25, 3.0) as f32;
        render::run(&self.host, root, self.shared.renderer.as_ref(), &path, scale)
    }

    fn run_project_tool(&mut self, name: &str, args: &Value, root: &ProjectRoot, shared: &Arc<Shared>) -> Result<Value, String> {
        let root = root.clone();
        let shared = Arc::clone(shared);
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
            "render_form" => {
                // The picture travels as image content: see `call_tool`.
                let (png, mut meta) = self.render(args, &root)?;
                meta["png_bytes"] = json!(png.len());
                Ok(meta)
            }
            _ => unreachable!("listed above"),
        }
    }
}

/// Tools that need no open project: the knowledge tools, and the two that
/// make or choose one.
pub fn project_free(name: &str) -> bool {
    name.starts_with("kb_") || name == "create_project" || name == "open_project"
}

/// An argument that must be an absolute path.
fn absolute_arg(args: &Value, key: &str, tool: &str) -> Result<std::path::PathBuf, String> {
    let raw = opt_str(args, key).ok_or_else(|| format!("{tool} needs a '{key}'"))?;
    let path = std::path::PathBuf::from(&raw);
    if !path.is_absolute() {
        return Err(format!("{tool}: '{key}' must be an absolute path"));
    }
    Ok(path)
}

/// Whether `asked` names the open project: its folder or any folder inside it
/// (a coding agent's working directory), its manifest, or its name.
fn names_project(root: &ProjectRoot, asked: &str) -> bool {
    let asked = asked.trim();
    let path = std::path::Path::new(asked);
    if path.is_absolute() || asked.contains('/') || asked.contains('\\') {
        let canon = |p: &std::path::Path| std::fs::canonicalize(p).ok();
        return match (canon(path), canon(root.dir())) {
            (Some(given), Some(dir)) => given.starts_with(&dir) || canon(root.manifest()) == Some(given),
            _ => false,
        };
    }
    let manifest = root.manifest_name();
    let stem = manifest
        .strip_suffix(".project.toml")
        .or_else(|| manifest.strip_suffix(".toml"))
        .unwrap_or(&manifest);
    asked.eq_ignore_ascii_case(&manifest) || asked.eq_ignore_ascii_case(stem)
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
        Self::tool_list_with_project()
    }

    fn call_tool(&mut self, name: &str, arguments: &Value) -> ToolResult {
        if name == "render_form" {
            return match self.render_form(arguments) {
                Ok((png, meta)) => ToolResult::ok(vec![Content::png(&png), Content::text(meta.to_string())]),
                Err(e) => ToolResult::failed(e),
            };
        }
        match self.call(name, arguments) {
            Ok(v) => ToolResult::ok(vec![Content::text(v.to_string())]),
            Err(e) => ToolResult::failed(e),
        }
    }

    fn capabilities(&self) -> Value {
        serde_json::json!({ "tools": { "listChanged": false }, "resources": { "listChanged": false } })
    }

    /// The rules, served live (spec 084 R11) — never copied into a project.
    fn instructions(&self) -> Option<String> {
        Some(crate::content::server_instructions(&self.host.version()))
    }

    /// The reference pack, served live from this binary (spec 084 R13, R14).
    fn list_resources(&mut self) -> Vec<cobolt_mcp::Resource> {
        crate::reference::pack(&self.host.version())
            .into_iter()
            .map(|d| cobolt_mcp::Resource {
                uri: format!("{}{}", crate::content::RESOURCE_PREFIX, d.name),
                name: d.name,
                title: Some(d.title),
                description: None,
                mime_type: Some("text/markdown".into()),
            })
            .collect()
    }

    fn read_resource(&mut self, uri: &str) -> Option<cobolt_mcp::ResourceContents> {
        let name = uri.strip_prefix(crate::content::RESOURCE_PREFIX)?;
        crate::reference::pack(&self.host.version())
            .into_iter()
            .find(|d| d.name == name)
            .map(|d| cobolt_mcp::ResourceContents {
                uri: uri.to_owned(),
                mime_type: Some("text/markdown".into()),
                text: d.body,
            })
    }
}
